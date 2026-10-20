use byteorder::{BigEndian, ByteOrder};
use crc32fast::Hasher;
use recovery_core::RecoveryError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairReport {
    pub original_size: usize,
    pub repaired_size: usize,
    pub detected_format: &'static str,
    pub issues_found: Vec<String>,
    pub fixes_applied: Vec<String>,
    #[serde(skip)]
    pub repaired_data: Vec<u8>,
    pub success: bool,
}

pub fn repair_image(data: &[u8], filename_hint: &str) -> Result<RepairReport, RecoveryError> {
    if data.len() < 4 {
        return Err(RecoveryError::RepairFailed("File data is too small to inspect".to_string()));
    }

    let ext = filename_hint
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    // JPEG Repair
    if data.starts_with(&[0xFF, 0xD8]) {
        return repair_jpeg(data, &ext);
    }

    // PNG Repair
    const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if data.len() >= 8 && data.starts_with(&PNG_SIG) {
        return repair_png(data, &ext);
    }

    Err(RecoveryError::RepairFailed(
        "Unsupported format for automated repair. Only JPEG and PNG formats are currently supported for structural reconstruction."
            .to_string(),
    ))
}

fn repair_jpeg(data: &[u8], ext: &str) -> Result<RepairReport, RecoveryError> {
    let mut issues = Vec::new();
    let mut fixes = Vec::new();
    let mut repaired = data.to_vec();

    if ext != "jpg" && ext != "jpeg" {
        issues.push(format!("File extension '.{ext}' does not match genuine JPEG SOI signature."));
        fixes.push("Recommended renaming extension to .jpg".to_string());
    }

    let has_eoi = data.len() >= 2 && data[data.len() - 2] == 0xFF && data[data.len() - 1] == 0xD9;

    if !has_eoi {
        issues.push("Truncated JPEG: Missing End-Of-Image (EOI) marker 0xFFD9".to_string());
        repaired.push(0xFF);
        repaired.push(0xD9);
        fixes.push("Appended valid 0xFFD9 EOI marker to close JPEG entropy stream".to_string());
    }

    let success = !fixes.is_empty();

    Ok(RepairReport {
        original_size: data.len(),
        repaired_size: repaired.len(),
        detected_format: "JPEG",
        issues_found: issues,
        fixes_applied: fixes,
        repaired_data: repaired,
        success,
    })
}

fn repair_png(data: &[u8], ext: &str) -> Result<RepairReport, RecoveryError> {
    let mut issues = Vec::new();
    let mut fixes = Vec::new();
    let mut repaired = data.to_vec();

    if ext != "png" {
        issues.push(format!("File extension '.{ext}' does not match genuine PNG header."));
        fixes.push("Recommended renaming extension to .png".to_string());
    }

    let mut offset = 8;
    let mut found_iend = false;
    let mut chunk_crcs_fixed = 0;

    while offset + 12 <= repaired.len() {
        let chunk_len = BigEndian::read_u32(&repaired[offset..offset + 4]) as usize;
        let mut chunk_type = [0u8; 4];
        chunk_type.copy_from_slice(&repaired[offset + 4..offset + 8]);

        if offset + 12 + chunk_len > repaired.len() {
            issues.push(format!(
                "Truncated chunk '{}' at byte offset {}",
                String::from_utf8_lossy(&chunk_type),
                offset
            ));
            break;
        }

        let recorded_crc = BigEndian::read_u32(&repaired[offset + 8 + chunk_len..offset + 12 + chunk_len]);

        let actual_crc = {
            let chunk_data = &repaired[offset + 8..offset + 8 + chunk_len];
            let mut hasher = Hasher::new();
            hasher.update(&chunk_type);
            hasher.update(chunk_data);
            hasher.finalize()
        };

        if recorded_crc != actual_crc {
            issues.push(format!(
                "Invalid CRC in chunk '{}' (expected 0x{:08X}, found 0x{:08X})",
                String::from_utf8_lossy(&chunk_type),
                actual_crc,
                recorded_crc
            ));
            BigEndian::write_u32(
                &mut repaired[offset + 8 + chunk_len..offset + 12 + chunk_len],
                actual_crc,
            );
            chunk_crcs_fixed += 1;
        }

        if &chunk_type == b"IEND" {
            found_iend = true;
            break;
        }

        offset += 12 + chunk_len;
    }

    if chunk_crcs_fixed > 0 {
        fixes.push(format!("Recalculated and patched {chunk_crcs_fixed} invalid chunk CRC checksums"));
    }

    if !found_iend {
        issues.push("Missing IEND terminating chunk".to_string());
        let iend_chunk = [0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
        repaired.extend_from_slice(&iend_chunk);
        fixes.push("Appended valid IEND termination chunk with standard CRC".to_string());
    }

    let success = !fixes.is_empty();

    Ok(RepairReport {
        original_size: data.len(),
        repaired_size: repaired.len(),
        detected_format: "PNG",
        issues_found: issues,
        fixes_applied: fixes,
        repaired_data: repaired,
        success,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_missing_eoi_repair() {
        let truncated_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00];
        let report = repair_image(&truncated_jpeg, "photo.jpg").expect("Repair should process JPEG");
        assert!(report.success);
        assert_eq!(report.issues_found.len(), 1);
        assert_eq!(report.repaired_data.len(), truncated_jpeg.len() + 2);
        assert_eq!(
            &report.repaired_data[report.repaired_data.len() - 2..],
            &[0xFF, 0xD9]
        );
    }

    #[test]
    fn test_png_corrupted_crc_repair() {
        let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let ihdr_data = [0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10, 0x08, 0x06, 0x00, 0x00, 0x00];
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&ihdr_data);
        png.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]); // Bad CRC!

        // IEND chunk
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]);

        let report = repair_image(&png, "image.png").expect("Repair should process PNG");
        assert!(report.success);
        assert!(report.fixes_applied.iter().any(|f| f.contains("Recalculated")));
    }
}
