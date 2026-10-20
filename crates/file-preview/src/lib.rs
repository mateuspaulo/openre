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
            if let Some(mut img_prev) = generate_image_preview(data) {
                img_prev.hex_dump = hex_lines;
                return img_prev;
            }
        }
        FileCategory::Text => {
            let max_text = data.len().min(4096);
            let snippet = String::from_utf8_lossy(&data[..max_text]).to_string();
            return PreviewData {
                preview_type: "text".to_string(),
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
            }
            return PreviewData {
                preview_type: "document".to_string(),
                thumbnail_base64: None,
                dimensions: None,
                hex_dump: hex_lines,
                text_snippet: None,
                metadata_entries: entries,
            };
        }
        FileCategory::Audio | FileCategory::Video => {
            return PreviewData {
                preview_type: "media".to_string(),
                thumbnail_base64: None,
                dimensions: None,
                hex_dump: hex_lines,
                text_snippet: None,
                metadata_entries: vec![
                    ("Container".to_string(), extension.to_uppercase()),
                    ("Total Bytes".to_string(), data.len().to_string()),
                ],
            };
        }
        _ => {}
    }

    PreviewData {
        preview_type: "hex".to_string(),
        thumbnail_base64: None,
        dimensions: None,
        hex_dump: hex_lines,
        text_snippet: None,
        metadata_entries: vec![("Raw Size".to_string(), format!("{} bytes", data.len()))],
    }
}

pub fn generate_image_preview(data: &[u8]) -> Option<PreviewData> {
    if data.len() < 16 {
        return None;
    }

    let cursor = Cursor::new(data);
    let reader = ImageReader::new(cursor).with_guessed_format().ok()?;
    let format = reader.format()?;
    let img = reader.decode().ok()?;

    let (w, h) = (img.width(), img.height());

    // Generate in-memory thumbnail (max 128x128)
    let thumb = img.resize(128, 128, FilterType::Triangle);
    let mut thumb_buf = Vec::new();
    let mut thumb_cursor = Cursor::new(&mut thumb_buf);
    thumb.write_to(&mut thumb_cursor, ImageFormat::Png).ok()?;

    let b64 = format!("data:image/png;base64,{}", BASE64.encode(&thumb_buf));

    Some(PreviewData {
        preview_type: "image".to_string(),
        thumbnail_base64: Some(b64),
        dimensions: Some((w, h)),
        hex_dump: Vec::new(),
        text_snippet: None,
        metadata_entries: vec![
            ("Format".to_string(), format!("{format:?}")),
            ("Resolution".to_string(), format!("{w} x {h} px")),
            ("Raw Size".to_string(), format!("{} bytes", data.len())),
        ],
    })
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
}
