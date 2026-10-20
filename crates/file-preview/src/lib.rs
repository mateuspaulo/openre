use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use image::imageops::FilterType;
use image::{ImageFormat, ImageReader};
use recovery_core::FileCategory;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexLine {
    pub offset: String,
    pub hex: String,
    pub ascii: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewData {
    pub preview_type: String,
    pub media_data_url: Option<String>,
    pub thumbnail_base64: Option<String>,
    pub dimensions: Option<(u32, u32)>,
    pub hex_dump: Vec<HexLine>,
    pub text_snippet: Option<String>,
    pub metadata_entries: Vec<(String, String)>,
}

pub fn generate_preview(data: &[u8], extension: &str) -> PreviewData {
    let category = FileCategory::from_extension(extension);
    let hex_lines = generate_hex_dump(data, 512);

    match category {
        FileCategory::Image => {
            if let Some(mut img_prev) = generate_image_preview(data, extension) {
                img_prev.hex_dump = hex_lines;
                return img_prev;
            }

            return PreviewData {
                preview_type: "image".to_string(),
                media_data_url: None,
                thumbnail_base64: None,
                dimensions: None,
                hex_dump: hex_lines,
                text_snippet: None,
                metadata_entries: vec![
                    ("Format".to_string(), extension.to_uppercase()),
                    ("Status".to_string(), "Incomplete or Corrupt Image Stream".to_string()),
                    ("Size".to_string(), format!("{} bytes", data.len())),
                ],
            };
        }
        FileCategory::Audio => {
            let mime = match extension.to_lowercase().as_str() {
                "mp3" => "audio/mpeg",
                "wav" => "audio/wav",
                "ogg" => "audio/ogg",
                "m4a" | "aac" => "audio/mp4",
                "flac" => "audio/flac",
                _ => "audio/mpeg",
            };

            // Encode data URL if file is under 15MB to support fallback playback without freezing IPC
            let media_url = if !data.is_empty() && data.len() <= 15 * 1024 * 1024 {
                Some(format!("data:{};base64,{}", mime, BASE64.encode(data)))
            } else {
                None
            };

            return PreviewData {
                preview_type: "audio".to_string(),
                media_data_url: media_url,
                thumbnail_base64: None,
                dimensions: None,
                hex_dump: hex_lines,
                text_snippet: None,
                metadata_entries: vec![
                    ("Type".to_string(), "Digital Audio Stream".to_string()),
                    ("Container".to_string(), extension.to_uppercase()),
                    ("MIME Type".to_string(), mime.to_string()),
                    ("Sample Size".to_string(), format!("{} bytes", data.len())),
                ],
            };
        }
        FileCategory::Video => {
            let mime = match extension.to_lowercase().as_str() {
                "mp4" | "m4v" => "video/mp4",
                "webm" => "video/webm",
                "mov" => "video/mp4",
                "mkv" => "video/mp4",
                "avi" => "video/x-msvideo",
                _ => "video/mp4",
            };

            // Encode data URL if file is under 15MB to support fallback playback without freezing IPC
            let media_url = if !data.is_empty() && data.len() <= 15 * 1024 * 1024 {
                Some(format!("data:{};base64,{}", mime, BASE64.encode(data)))
            } else {
                None
            };

            return PreviewData {
                preview_type: "video".to_string(),
                media_data_url: media_url,
                thumbnail_base64: None,
                dimensions: None,
                hex_dump: hex_lines,
                text_snippet: None,
                metadata_entries: vec![
                    ("Type".to_string(), "Digital Video Container".to_string()),
                    ("Container".to_string(), extension.to_uppercase()),
                    ("MIME Type".to_string(), mime.to_string()),
                    ("Stream Size".to_string(), format!("{} bytes", data.len())),
                ],
            };
        }
        FileCategory::Text => {
            let max_text = data.len().min(32768);
            let snippet = String::from_utf8_lossy(&data[..max_text]).to_string();
            return PreviewData {
                preview_type: "text".to_string(),
                media_data_url: None,
                thumbnail_base64: None,
                dimensions: None,
                hex_dump: hex_lines,
                text_snippet: Some(snippet),
                metadata_entries: vec![
                    ("Detected Encoding".to_string(), "UTF-8 / ASCII".to_string()),
                    ("Length Bytes".to_string(), data.len().to_string()),
                ],
            };
        }
        FileCategory::Document => {
            let mut entries = vec![("Format".to_string(), extension.to_uppercase())];
            if extension.eq_ignore_ascii_case("pdf") {
                entries.push(("Type".to_string(), "Adobe Portable Document Format".to_string()));
            } else if extension.eq_ignore_ascii_case("docx") {
                entries.push(("Type".to_string(), "Microsoft Word Document (OpenXML)".to_string()));
            } else if extension.eq_ignore_ascii_case("xlsx") {
                entries.push(("Type".to_string(), "Microsoft Excel Spreadsheet (OpenXML)".to_string()));
            } else if extension.eq_ignore_ascii_case("zip") {
                entries.push(("Type".to_string(), "Compressed Archive (ZIP)".to_string()));
            }

            return PreviewData {
                preview_type: "document".to_string(),
                media_data_url: None,
                thumbnail_base64: None,
                dimensions: None,
                hex_dump: hex_lines,
                text_snippet: None,
                metadata_entries: entries,
            };
        }
        _ => {}
    }

    PreviewData {
        preview_type: "hex".to_string(),
        media_data_url: None,
        thumbnail_base64: None,
        dimensions: None,
        hex_dump: hex_lines,
        text_snippet: None,
        metadata_entries: vec![("Raw Size".to_string(), format!("{} bytes", data.len()))],
    }
}

fn try_decode_image(data: &[u8], extension: &str) -> Option<image::DynamicImage> {
    if data.len() < 16 {
        return None;
    }

    // Attempt 1: Direct decode with format guessing
    if let Ok(reader) = ImageReader::new(Cursor::new(data)).with_guessed_format() {
        if let Ok(img) = reader.decode() {
            return Some(img);
        }
    }

    // Attempt 2: Explicit format from extension
    if let Some(fmt) = ImageFormat::from_extension(extension) {
        let mut r = ImageReader::new(Cursor::new(data));
        r.set_format(fmt);
        if let Ok(img) = r.decode() {
            return Some(img);
        }
    }

    // Attempt 3: Truncated stream repair
    // JPEG missing EOI (FF D9)
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut patched = data.to_vec();
        if patched.len() >= 2 && !(patched[patched.len() - 2] == 0xFF && patched[patched.len() - 1] == 0xD9) {
            patched.push(0xFF);
            patched.push(0xD9);
            let mut r = ImageReader::new(Cursor::new(&patched));
            r.set_format(ImageFormat::Jpeg);
            if let Ok(img) = r.decode() {
                return Some(img);
            }
        }
    }

    // PNG missing IEND
    const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if data.len() >= 8 && data.starts_with(&PNG_SIG) {
        let mut patched = data.to_vec();
        let iend_chunk = [0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
        patched.extend_from_slice(&iend_chunk);
        let mut r = ImageReader::new(Cursor::new(&patched));
        r.set_format(ImageFormat::Png);
        if let Ok(img) = r.decode() {
            return Some(img);
        }
    }

    None
}

pub fn generate_image_preview(data: &[u8], extension: &str) -> Option<PreviewData> {
    let img = try_decode_image(data, extension)?;
    let (w, h) = (img.width(), img.height());

    // Generate 80x80 compact thumbnail (max 1.5 - 2KB)
    let thumb = img.resize(80, 80, FilterType::Triangle);
    let mut thumb_buf = Vec::new();
    let thumb_b64 = if thumb.write_to(&mut Cursor::new(&mut thumb_buf), ImageFormat::Jpeg).is_ok() {
        format!("data:image/jpeg;base64,{}", BASE64.encode(&thumb_buf))
    } else {
        let mut png_buf = Vec::new();
        thumb.write_to(&mut Cursor::new(&mut png_buf), ImageFormat::Png).ok()?;
        format!("data:image/png;base64,{}", BASE64.encode(&png_buf))
    };

    // Generate max 600x600 preview for the lateral drawer (approx 25KB - 35KB)
    let preview_img = if w > 600 || h > 600 {
        img.resize(600, 600, FilterType::Triangle)
    } else {
        img
    };
    let mut prev_buf = Vec::new();
    let media_url = if preview_img.write_to(&mut Cursor::new(&mut prev_buf), ImageFormat::Jpeg).is_ok() {
        Some(format!("data:image/jpeg;base64,{}", BASE64.encode(&prev_buf)))
    } else {
        let mut png_buf = Vec::new();
        if preview_img.write_to(&mut Cursor::new(&mut png_buf), ImageFormat::Png).is_ok() {
            Some(format!("data:image/png;base64,{}", BASE64.encode(&png_buf)))
        } else {
            Some(thumb_b64.clone())
        }
    };

    Some(PreviewData {
        preview_type: "image".to_string(),
        media_data_url: media_url,
        thumbnail_base64: Some(thumb_b64),
        dimensions: Some((w, h)),
        hex_dump: Vec::new(),
        text_snippet: None,
        metadata_entries: vec![
            ("Format".to_string(), extension.to_uppercase()),
            ("Resolution".to_string(), format!("{w} × {h} px")),
            ("Raw Size".to_string(), format!("{} bytes", data.len())),
        ],
    })
}

pub fn generate_thumbnail_data_url(data: &[u8], extension: &str) -> Option<String> {
    let img = try_decode_image(data, extension)?;
    let thumb = img.resize(80, 80, FilterType::Triangle);
    let mut thumb_buf = Vec::new();
    if thumb.write_to(&mut Cursor::new(&mut thumb_buf), ImageFormat::Jpeg).is_ok() {
        Some(format!("data:image/jpeg;base64,{}", BASE64.encode(&thumb_buf)))
    } else {
        let mut png_buf = Vec::new();
        thumb.write_to(&mut Cursor::new(&mut png_buf), ImageFormat::Png).ok()?;
        Some(format!("data:image/png;base64,{}", BASE64.encode(&png_buf)))
    }
}

pub fn generate_hex_dump(data: &[u8], max_bytes: usize) -> Vec<HexLine> {
    let limit = data.len().min(max_bytes);
    let slice = &data[..limit];
    let mut lines = Vec::new();

    for (chunk_idx, chunk) in slice.chunks(16).enumerate() {
        let offset_str = format!("{:08X}", chunk_idx * 16);
        let mut hex_parts = Vec::new();
        let mut ascii_chars = String::new();

        for &b in chunk {
            hex_parts.push(format!("{:02X}", b));
            if (0x20..=0x7E).contains(&b) {
                ascii_chars.push(b as char);
            } else {
                ascii_chars.push('.');
            }
        }

        // Pad if less than 16 bytes in last chunk
        while hex_parts.len() < 16 {
            hex_parts.push("  ".to_string());
        }

        let hex_formatted = format!(
            "{}  {}",
            hex_parts[0..8].join(" "),
            hex_parts[8..16].join(" ")
        );

        lines.push(HexLine {
            offset: offset_str,
            hex: hex_formatted,
            ascii: ascii_chars,
        });
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_dump_generation() {
        let data = b"Hello OpenRe Data Recovery!";
        let lines = generate_hex_dump(data, 100);
        assert!(!lines.is_empty());
        assert_eq!(lines[0].offset, "00000000");
        assert!(lines[0].ascii.contains("Hello OpenRe"));
    }

    #[test]
    fn test_text_preview() {
        let text = b"This is a recovery test file.";
        let prev = generate_preview(text, "txt");
        assert_eq!(prev.preview_type, "text");
        assert_eq!(prev.text_snippet.unwrap(), "This is a recovery test file.");
    }

    #[test]
    fn test_corrupt_data_does_not_produce_broken_thumbnail() {
        // Raw random bytes should return None and NEVER return a broken base64 string
        let garbage = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let thumb = generate_thumbnail_data_url(&garbage, "jpg");
        assert!(thumb.is_none());

        let prev = generate_preview(&garbage, "jpg");
        assert_eq!(prev.preview_type, "image");
        assert!(prev.thumbnail_base64.is_none());
        assert!(prev.media_data_url.is_none());
    }

    #[test]
    fn test_valid_image_thumbnail_generation() {
        // Create a 10x10 test image and encode as PNG
        let img = image::RgbImage::new(10, 10);
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png).unwrap();

        let thumb = generate_thumbnail_data_url(&buf, "png");
        assert!(thumb.is_some());
        let url = thumb.unwrap();
        assert!(url.starts_with("data:image/"));
        // Thumbnail should be compact (< 4KB)
        assert!(url.len() < 4096);
    }
}
