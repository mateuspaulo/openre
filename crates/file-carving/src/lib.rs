use byteorder::{BigEndian, ByteOrder, LittleEndian};
use crc32fast::Hasher;
use recovery_core::{FileCategory, IntegrityStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarveResult {
    pub extension: &'static str,
    pub category: FileCategory,
    pub length_bytes: u64,
    pub integrity: IntegrityStatus,
}

pub fn carve_at_offset(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 16 {
        return None;
    }

    if let Some(res) = carve_jpeg(data) {
        return Some(res);
    }
    if let Some(res) = carve_png(data) {
        return Some(res);
    }
    if let Some(res) = carve_gif(data) {
        return Some(res);
    }
    if let Some(res) = carve_bmp(data) {
        return Some(res);
    }
    if let Some(res) = carve_webp(data) {
        return Some(res);
    }
    if let Some(res) = carve_tiff(data) {
        return Some(res);
    }
    if let Some(res) = carve_pdf(data) {
        return Some(res);
    }
    if let Some(res) = carve_zip_or_office(data) {
        return Some(res);
    }
    if let Some(res) = carve_wav(data) {
        return Some(res);
    }
    if let Some(res) = carve_mp3(data) {
        return Some(res);
    }
    if let Some(res) = carve_mp4(data) {
        return Some(res);
    }
    if let Some(res) = carve_mkv(data) {
        return Some(res);
    }

    None
}

/// Carves JPEG / JPG files: Starts with FF D8 FF, traverses markers until FF D9 (EOI).
fn carve_jpeg(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 || data[2] != 0xFF {
        return None;
    }

    let mut i = 2;
    let max_len = data.len().min(50 * 1024 * 1024); // Cap search at 50MB

    while i + 1 < max_len {
        if data[i] == 0xFF && data[i + 1] == 0xD9 {
            return Some(CarveResult {
                extension: "jpg",
                category: FileCategory::Image,
                length_bytes: (i + 2) as u64,
                integrity: IntegrityStatus::LikelyRecoverable,
            });
        }
        i += 1;
    }

    // Found SOI but no EOI within buffer -> partially recoverable with padding detection
    if data.len() >= 512 {
        let mut end = 512;
        let mut zero_run = 0;
        for (idx, &b) in data[..data.len().min(16384)].iter().enumerate() {
            if b == 0 {
                zero_run += 1;
                if zero_run >= 512 {
                    end = idx.saturating_sub(512).max(512);
                    break;
                }
            } else {
                zero_run = 0;
                end = (idx + 1).max(512);
            }
        }
        Some(CarveResult {
            extension: "jpg",
            category: FileCategory::Image,
            length_bytes: end as u64,
            integrity: IntegrityStatus::PartiallyRecoverable,
        })
    } else {
        None
    }
}

/// Carves PNG files: Header 89 50 4E 47 0D 0A 1A 0A, iterates chunks with CRC32 checks, ends on IEND.
fn carve_png(data: &[u8]) -> Option<CarveResult> {
    const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if data.len() < 8 || data[0..8] != PNG_SIG {
        return None;
    }

    let mut offset = 8;
    let mut crc_errors = 0;
    let mut found_iend = false;
    let max_len = data.len().min(50 * 1024 * 1024);

    while offset + 12 <= max_len {
        let chunk_len = BigEndian::read_u32(&data[offset..offset + 4]) as usize;
        let chunk_type = &data[offset + 4..offset + 8];

        if offset + 12 + chunk_len > max_len {
            break;
        }

        let chunk_data = &data[offset + 8..offset + 8 + chunk_len];
        let expected_crc = BigEndian::read_u32(&data[offset + 8 + chunk_len..offset + 12 + chunk_len]);

        let mut hasher = Hasher::new();
        hasher.update(chunk_type);
        hasher.update(chunk_data);
        if hasher.finalize() != expected_crc {
            crc_errors += 1;
        }

        offset += 12 + chunk_len;

        if chunk_type == b"IEND" {
            found_iend = true;
            break;
        }
    }

    if found_iend {
        let integrity = if crc_errors == 0 {
            IntegrityStatus::LikelyRecoverable
        } else {
            IntegrityStatus::PartiallyRecoverable
        };
        Some(CarveResult {
            extension: "png",
            category: FileCategory::Image,
            length_bytes: offset as u64,
            integrity,
        })
    } else if offset > 8 {
        Some(CarveResult {
            extension: "png",
            category: FileCategory::Image,
            length_bytes: offset as u64,
            integrity: IntegrityStatus::PartiallyRecoverable,
        })
    } else {
        None
    }
}

/// Carves GIF files: GIF87a or GIF89a, ends on 0x3B.
fn carve_gif(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 6 {
        return None;
    }
    if &data[0..6] != b"GIF87a" && &data[0..6] != b"GIF89a" {
        return None;
    }

    let max_len = data.len().min(30 * 1024 * 1024);
    for i in (6..max_len).rev() {
        if data[i] == 0x3B {
            return Some(CarveResult {
                extension: "gif",
                category: FileCategory::Image,
                length_bytes: (i + 1) as u64,
                integrity: IntegrityStatus::LikelyRecoverable,
            });
        }
    }

    Some(CarveResult {
        extension: "gif",
        category: FileCategory::Image,
        length_bytes: data.len() as u64,
        integrity: IntegrityStatus::StructureDetectedUnverified,
    })
}

/// Carves BMP files: 'BM' signature with little-endian file size at offset 2.
fn carve_bmp(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 14 || data[0] != 0x42 || data[1] != 0x4D {
        return None;
    }

    let file_size = LittleEndian::read_u32(&data[2..6]) as u64;
    if file_size < 14 || file_size > 100 * 1024 * 1024 {
        return None;
    }

    let integrity = if (file_size as usize) <= data.len() {
        IntegrityStatus::LikelyRecoverable
    } else {
        IntegrityStatus::PartiallyRecoverable
    };

    Some(CarveResult {
        extension: "bmp",
        category: FileCategory::Image,
        length_bytes: file_size.min(data.len() as u64),
        integrity,
    })
}

/// Carves WebP files: RIFF....WEBP.
fn carve_webp(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }

    let riff_size = LittleEndian::read_u32(&data[4..8]) as u64;
    let total_size = riff_size + 8;

    let integrity = if (total_size as usize) <= data.len() {
        IntegrityStatus::LikelyRecoverable
    } else {
        IntegrityStatus::PartiallyRecoverable
    };

    Some(CarveResult {
        extension: "webp",
        category: FileCategory::Image,
        length_bytes: total_size.min(data.len() as u64),
        integrity,
    })
}

/// Carves TIFF files: 'II*\0' or 'MM\0*'.
fn carve_tiff(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 8 {
        return None;
    }
    let is_little = &data[0..4] == b"II*\0";
    let is_big = &data[0..4] == b"MM\0*";

    if !is_little && !is_big {
        return None;
    }

    Some(CarveResult {
        extension: "tiff",
        category: FileCategory::Image,
        length_bytes: data.len().min(50 * 1024 * 1024) as u64,
        integrity: IntegrityStatus::StructureDetectedUnverified,
    })
}

/// Carves PDF documents: %PDF-1.x, checks for %%EOF.
fn carve_pdf(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 8 || &data[0..5] != b"%PDF-" {
        return None;
    }

    let max_len = data.len().min(100 * 1024 * 1024);
    let search_window = &data[..max_len];

    // Search backwards for %%EOF
    for i in (0..search_window.len().saturating_sub(5)).rev() {
        if &search_window[i..i + 5] == b"%%EOF" {
            let mut end = i + 5;
            while end < search_window.len() && (search_window[end] == b'\r' || search_window[end] == b'\n') {
                end += 1;
            }
            return Some(CarveResult {
                extension: "pdf",
                category: FileCategory::Document,
                length_bytes: end as u64,
                integrity: IntegrityStatus::LikelyRecoverable,
            });
        }
    }

    Some(CarveResult {
        extension: "pdf",
        category: FileCategory::Document,
        length_bytes: search_window.len() as u64,
        integrity: IntegrityStatus::PartiallyRecoverable,
    })
}

/// Carves ZIP and Microsoft Office OpenXML files (DOCX, XLSX, PPTX).
fn carve_zip_or_office(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 30 || &data[0..4] != b"PK\x03\x04" {
        return None;
    }

    let max_len = data.len().min(200 * 1024 * 1024);
    let search_slice = &data[..max_len];

    // Look for End of Central Directory (EOCD): PK\x05\x06
    let mut eocd_pos = None;
    for i in (0..search_slice.len().saturating_sub(22)).rev() {
        if &search_slice[i..i + 4] == b"PK\x05\x06" {
            let comment_len = LittleEndian::read_u16(&search_slice[i + 20..i + 22]) as usize;
            let full_len = i + 22 + comment_len;
            if full_len <= search_slice.len() {
                eocd_pos = Some(full_len);
                break;
            }
        }
    }

    let total_len = eocd_pos.unwrap_or(search_slice.len());
    let content_slice = &search_slice[..total_len];

    // Determine specific Office OpenXML format
    let (extension, category) = if find_subsequence(content_slice, b"word/document.xml") {
        ("docx", FileCategory::Document)
    } else if find_subsequence(content_slice, b"xl/workbook.xml") {
        ("xlsx", FileCategory::Document)
    } else if find_subsequence(content_slice, b"ppt/presentation.xml") {
        ("pptx", FileCategory::Document)
    } else {
        ("zip", FileCategory::Archive)
    };

    let integrity = if eocd_pos.is_some() {
        IntegrityStatus::LikelyRecoverable
    } else {
        IntegrityStatus::PartiallyRecoverable
    };

    Some(CarveResult {
        extension,
        category,
        length_bytes: total_len as u64,
        integrity,
    })
}

/// Carves WAV audio files: RIFF....WAVEfmt .
fn carve_wav(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 16 || &data[0..4] != b"RIFF" || &data[8..16] != b"WAVEfmt " {
        return None;
    }

    let riff_size = LittleEndian::read_u32(&data[4..8]) as u64;
    let total_size = riff_size + 8;

    let integrity = if (total_size as usize) <= data.len() {
        IntegrityStatus::LikelyRecoverable
    } else {
        IntegrityStatus::PartiallyRecoverable
    };

    Some(CarveResult {
        extension: "wav",
        category: FileCategory::Audio,
        length_bytes: total_size.min(data.len() as u64),
        integrity,
    })
}

/// Carves MP3 files: ID3 header or sync frames.
fn carve_mp3(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 10 {
        return None;
    }

    if &data[0..3] == b"ID3" {
        // ID3v2 syncsafe size at offset 6..10
        let b0 = data[6] as u32;
        let b1 = data[7] as u32;
        let b2 = data[8] as u32;
        let b3 = data[9] as u32;
        if b0 < 128 && b1 < 128 && b2 < 128 && b3 < 128 {
            let id3_size = (b0 << 21) | (b1 << 14) | (b2 << 7) | b3;
            let total_header = id3_size + 10;
            return Some(CarveResult {
                extension: "mp3",
                category: FileCategory::Audio,
                length_bytes: data.len().min(20 * 1024 * 1024) as u64,
                integrity: if (total_header as usize) <= data.len() {
                    IntegrityStatus::LikelyRecoverable
                } else {
                    IntegrityStatus::PartiallyRecoverable
                },
            });
        }
    }

    // MPEG Audio frame sync (0xFFE or 0xFFF)
    if data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 {
        return Some(CarveResult {
            extension: "mp3",
            category: FileCategory::Audio,
            length_bytes: data.len().min(15 * 1024 * 1024) as u64,
            integrity: IntegrityStatus::StructureDetectedUnverified,
        });
    }

    None
}

/// Carves MP4 / MOV videos: ftyp box traversal.
fn carve_mp4(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 12 {
        return None;
    }

    let box_len = BigEndian::read_u32(&data[0..4]) as usize;
    let box_type = &data[4..8];

    if box_type == b"ftyp" && box_len >= 8 && box_len <= data.len() {
        let brand = &data[8..12.min(data.len())];
        let extension = if brand == b"qt  " { "mov" } else { "mp4" };

        return Some(CarveResult {
            extension,
            category: FileCategory::Video,
            length_bytes: data.len().min(500 * 1024 * 1024) as u64,
            integrity: IntegrityStatus::LikelyRecoverable,
        });
    }

    None
}

/// Carves MKV / WebM videos: EBML header 0x1A45DFA3.
fn carve_mkv(data: &[u8]) -> Option<CarveResult> {
    if data.len() < 4 || &data[0..4] != [0x1A, 0x45, 0xDF, 0xA3] {
        return None;
    }

    Some(CarveResult {
        extension: "mkv",
        category: FileCategory::Video,
        length_bytes: data.len().min(500 * 1024 * 1024) as u64,
        integrity: IntegrityStatus::LikelyRecoverable,
    })
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_carve_jpeg_valid() {
        let mut jpeg_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        jpeg_bytes.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        jpeg_bytes.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let res = carve_at_offset(&jpeg_bytes).expect("JPEG should be carved");
        assert_eq!(res.extension, "jpg");
        assert_eq!(res.category, FileCategory::Image);
        assert_eq!(res.integrity, IntegrityStatus::LikelyRecoverable);
        assert_eq!(res.length_bytes, jpeg_bytes.len() as u64);
    }

    #[test]
    fn test_carve_png_valid() {
        let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG sig
        // IHDR chunk
        let ihdr_data = [0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10, 0x08, 0x06, 0x00, 0x00, 0x00];
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // 13 bytes
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&ihdr_data);
        let mut h = Hasher::new();
        h.update(b"IHDR");
        h.update(&ihdr_data);
        png.extend_from_slice(&h.finalize().to_be_bytes());

        // IEND chunk
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        png.extend_from_slice(b"IEND");
        let mut h_iend = Hasher::new();
        h_iend.update(b"IEND");
        png.extend_from_slice(&h_iend.finalize().to_be_bytes());

        let res = carve_at_offset(&png).expect("PNG should be carved");
        assert_eq!(res.extension, "png");
        assert_eq!(res.integrity, IntegrityStatus::LikelyRecoverable);
        assert_eq!(res.length_bytes, png.len() as u64);
    }

    #[test]
    fn test_carve_pdf() {
        let pdf = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n%%EOF\n".to_vec();
        let res = carve_at_offset(&pdf).expect("PDF should be carved");
        assert_eq!(res.extension, "pdf");
        assert_eq!(res.category, FileCategory::Document);
        assert_eq!(res.integrity, IntegrityStatus::LikelyRecoverable);
    }

    #[test]
    fn test_carve_bmp() {
        let mut bmp = vec![0x42, 0x4D];
        let file_size: u32 = 120;
        bmp.extend_from_slice(&file_size.to_le_bytes());
        bmp.resize(120, 0);

        let res = carve_at_offset(&bmp).expect("BMP should be carved");
        assert_eq!(res.extension, "bmp");
        assert_eq!(res.length_bytes, 120);
        assert_eq!(res.integrity, IntegrityStatus::LikelyRecoverable);
    }
}
