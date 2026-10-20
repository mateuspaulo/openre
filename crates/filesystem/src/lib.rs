use byteorder::{ByteOrder, LittleEndian};
use recovery_core::{FileCategory, IntegrityStatus, MetadataSource, RecoveredFile};

#[derive(Debug, Clone)]
pub struct DiscoveredFilesystemFile {
    pub filename: String,
    pub original_path: Option<String>,
    pub size_bytes: u64,
    pub is_deleted: bool,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub source: MetadataSource,
    pub cluster_or_sector_offset: u64,
    pub resident_data: Option<Vec<u8>>,
}

impl DiscoveredFilesystemFile {
    pub fn to_recovered_file(&self, id: String) -> RecoveredFile {
        let ext = self
            .filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase();
        let category = FileCategory::from_extension(&ext);
        let integrity = if self.size_bytes > 0 {
            IntegrityStatus::LikelyRecoverable
        } else {
            IntegrityStatus::StructureDetectedUnverified
        };

        RecoveredFile {
            id,
            filename: self.filename.clone(),
            original_path: self.original_path.clone(),
            category,
            extension: ext,
            size_bytes: self.size_bytes,
            source_offset: self.cluster_or_sector_offset,
            integrity,
            created_at: self.created_at.clone(),
            modified_at: self.modified_at.clone(),
            metadata_source: self.source,
            sha256: None,
        }
    }
}

pub fn detect_filesystem(sector_0: &[u8]) -> Option<&'static str> {
    if sector_0.len() < 512 {
        return None;
    }

    if &sector_0[3..11] == b"NTFS    " {
        return Some("NTFS");
    }
    if &sector_0[3..11] == b"EXFAT   " {
        return Some("exFAT");
    }
    if &sector_0[82..90] == b"FAT32   " {
        return Some("FAT32");
    }
    if sector_0.len() >= 1080 && LittleEndian::read_u16(&sector_0[1024 + 56..1024 + 58]) == 0xEF53 {
        return Some("ext4");
    }

    None
}

// -----------------------------------------------------------------------------
// NTFS MFT Parser
// -----------------------------------------------------------------------------
pub mod ntfs {
    use super::*;

    pub fn parse_mft_record(record: &[u8]) -> Option<DiscoveredFilesystemFile> {
        if record.len() < 1024 || &record[0..4] != b"FILE" {
            return None;
        }

        let flags = LittleEndian::read_u16(&record[0x16..0x18]);
        let is_in_use = (flags & 0x01) != 0;
        let is_directory = (flags & 0x02) != 0;

        if is_directory {
            return None;
        }

        let mut attr_offset = LittleEndian::read_u16(&record[0x14..0x16]) as usize;
        let mut filename = None;
        let mut file_size = 0u64;
        let mut resident_data = None;

        while attr_offset + 8 <= record.len() {
            let attr_type = LittleEndian::read_u32(&record[attr_offset..attr_offset + 4]);
            if attr_type == 0xFFFFFFFF || attr_type == 0 {
                break;
            }

            let attr_len = LittleEndian::read_u32(&record[attr_offset + 4..attr_offset + 8]) as usize;
            if attr_len == 0 || attr_offset + attr_len > record.len() {
                break;
            }

            let is_non_resident = record[attr_offset + 8] != 0;

            match attr_type {
                0x30 => {
                    // $FILE_NAME
                    if !is_non_resident && attr_offset + 24 <= record.len() {
                        let content_offset =
                            LittleEndian::read_u16(&record[attr_offset + 20..attr_offset + 22]) as usize;
                        let fn_body = attr_offset + content_offset;
                        if fn_body + 66 <= record.len() {
                            let fn_len = record[fn_body + 64] as usize;
                            let fn_chars_start = fn_body + 66;
                            if fn_chars_start + fn_len * 2 <= record.len() {
                                let u16_chars: Vec<u16> = (0..fn_len)
                                    .map(|i| {
                                        LittleEndian::read_u16(
                                            &record[fn_chars_start + i * 2..fn_chars_start + (i + 1) * 2],
                                        )
                                    })
                                    .collect();
                                if let Ok(name) = String::from_utf16(&u16_chars) {
                                    if !name.starts_with('$') {
                                        filename = Some(name);
                                    }
                                }
                            }
                        }
                    }
                }
                0x80 => {
                    // $DATA
                    if is_non_resident {
                        if attr_offset + 56 <= record.len() {
                            file_size = LittleEndian::read_u64(&record[attr_offset + 48..attr_offset + 56]);
                        }
                    } else if attr_offset + 24 <= record.len() {
                        let content_len =
                            LittleEndian::read_u32(&record[attr_offset + 16..attr_offset + 20]) as usize;
                        let content_offset =
                            LittleEndian::read_u16(&record[attr_offset + 20..attr_offset + 22]) as usize;
                        file_size = content_len as u64;
                        if attr_offset + content_offset + content_len <= record.len() {
                            resident_data = Some(
                                record[attr_offset + content_offset..attr_offset + content_offset + content_len]
                                    .to_vec(),
                            );
                        }
                    }
                }
                _ => {}
            }

            attr_offset += attr_len;
        }

        filename.map(|name| DiscoveredFilesystemFile {
            filename: name,
            original_path: None,
            size_bytes: file_size,
            is_deleted: !is_in_use,
            created_at: None,
            modified_at: None,
            source: MetadataSource::FilesystemNtfs,
            cluster_or_sector_offset: 0,
            resident_data,
        })
    }
}

// -----------------------------------------------------------------------------
// FAT32 Directory Entry Parser
// -----------------------------------------------------------------------------
pub mod fat32 {
    use super::*;

    pub fn parse_fat32_entries(directory_block: &[u8]) -> Vec<DiscoveredFilesystemFile> {
        let mut results = Vec::new();
        let mut lfn_parts: Vec<(u8, String)> = Vec::new();

        for chunk in directory_block.chunks_exact(32) {
            let first_byte = chunk[0];
            if first_byte == 0x00 {
                // End of directory
                break;
            }

            let attr = chunk[11];

            // Long File Name (LFN) Entry
            if attr == 0x0F {
                let seq = first_byte;
                let mut u16_chars = Vec::new();
                for &pair in &[
                    &chunk[1..11],
                    &chunk[14..26],
                    &chunk[28..32],
                ] {
                    for c in pair.chunks_exact(2) {
                        let val = LittleEndian::read_u16(c);
                        if val != 0x0000 && val != 0xFFFF {
                            u16_chars.push(val);
                        }
                    }
                }
                if let Ok(part) = String::from_utf16(&u16_chars) {
                    lfn_parts.push((seq, part));
                }
                continue;
            }

            // Normal Directory Entry
            let is_deleted = first_byte == 0xE5;
            let is_dir = (attr & 0x10) != 0;
            let is_volume_id = (attr & 0x08) != 0;

            if is_dir || is_volume_id {
                lfn_parts.clear();
                continue;
            }

            let filename = if !lfn_parts.is_empty() {
                lfn_parts.sort_by(|a, b| b.0.cmp(&a.0)); // Reconstruct reverse sequence
                let full: String = lfn_parts.iter().map(|(_, s)| s.as_str()).collect();
                lfn_parts.clear();
                full
            } else {
                let name_part = String::from_utf8_lossy(&chunk[0..8]).trim().to_string();
                let ext_part = String::from_utf8_lossy(&chunk[8..11]).trim().to_string();
                if ext_part.is_empty() {
                    name_part
                } else {
                    format!("{name_part}.{ext_part}")
                }
            };

            let high_cluster = LittleEndian::read_u16(&chunk[20..22]) as u32;
            let low_cluster = LittleEndian::read_u16(&chunk[26..28]) as u32;
            let start_cluster = (high_cluster << 16) | low_cluster;
            let file_size = LittleEndian::read_u32(&chunk[28..32]) as u64;

            if !filename.is_empty() {
                results.push(DiscoveredFilesystemFile {
                    filename,
                    original_path: None,
                    size_bytes: file_size,
                    is_deleted,
                    created_at: None,
                    modified_at: None,
                    source: MetadataSource::FilesystemFat32,
                    cluster_or_sector_offset: start_cluster as u64,
                    resident_data: None,
                });
            }
        }

        results
    }
}

// -----------------------------------------------------------------------------
// exFAT Directory Entry Parser
// -----------------------------------------------------------------------------
pub mod exfat {
    use super::*;

    pub fn parse_exfat_entries(dir_block: &[u8]) -> Vec<DiscoveredFilesystemFile> {
        let mut results = Vec::new();

        let mut current_name = String::new();
        let mut file_size = 0u64;
        let mut is_deleted = false;
        let mut start_cluster = 0u64;

        for chunk in dir_block.chunks_exact(32) {
            let entry_type = chunk[0];
            if entry_type == 0x00 {
                break;
            }

            match entry_type {
                0x85 | 0x05 => {
                    // 0x85 = File entry, 0x05 = Deleted file entry
                    is_deleted = entry_type == 0x05;
                    current_name.clear();
                }
                0xC0 => {
                    // Stream extension
                    file_size = LittleEndian::read_u64(&chunk[24..32]);
                    start_cluster = LittleEndian::read_u32(&chunk[20..24]) as u64;
                }
                0xC1 => {
                    // File name entry
                    let u16_chars: Vec<u16> = chunk[2..32]
                        .chunks_exact(2)
                        .map(LittleEndian::read_u16)
                        .filter(|&c| c != 0x0000)
                        .collect();
                    if let Ok(part) = String::from_utf16(&u16_chars) {
                        current_name.push_str(&part);
                    }

                    if !current_name.is_empty() {
                        results.push(DiscoveredFilesystemFile {
                            filename: current_name.clone(),
                            original_path: None,
                            size_bytes: file_size,
                            is_deleted,
                            created_at: None,
                            modified_at: None,
                            source: MetadataSource::FilesystemExFat,
                            cluster_or_sector_offset: start_cluster,
                            resident_data: None,
                        });
                        current_name.clear();
                    }
                }
                _ => {}
            }
        }

        results
    }
}

// -----------------------------------------------------------------------------
// ext4 Inode Parser
// -----------------------------------------------------------------------------
pub mod ext4 {
    use super::*;

    pub fn parse_ext4_inode(inode: &[u8], inode_num: u32) -> Option<DiscoveredFilesystemFile> {
        if inode.len() < 128 {
            return None;
        }

        let mode = LittleEndian::read_u16(&inode[0..2]);
        let is_regular_file = (mode & 0xF000) == 0x8000;
        if !is_regular_file {
            return None;
        }

        let size_low = LittleEndian::read_u32(&inode[4..8]) as u64;
        let size_high = LittleEndian::read_u32(&inode[108..112]) as u64;
        let total_size = (size_high << 32) | size_low;
        let links_count = LittleEndian::read_u16(&inode[26..28]);
        let is_deleted = links_count == 0;

        Some(DiscoveredFilesystemFile {
            filename: format!("inode_{inode_num}.dat"),
            original_path: None,
            size_bytes: total_size,
            is_deleted,
            created_at: None,
            modified_at: None,
            source: MetadataSource::FilesystemExt4,
            cluster_or_sector_offset: inode_num as u64,
            resident_data: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_detection() {
        let mut ntfs_sector = [0u8; 512];
        ntfs_sector[3..11].copy_from_slice(b"NTFS    ");
        assert_eq!(detect_filesystem(&ntfs_sector), Some("NTFS"));

        let mut fat32_sector = [0u8; 512];
        fat32_sector[82..90].copy_from_slice(b"FAT32   ");
        assert_eq!(detect_filesystem(&fat32_sector), Some("FAT32"));
    }

    #[test]
    fn test_fat32_deleted_entry_parsing() {
        let mut dir_block = vec![0u8; 64];
        // Entry 1: Deleted file with 0xE5
        dir_block[0] = 0xE5;
        dir_block[1..8].copy_from_slice(b"TESTDOC");
        dir_block[8..11].copy_from_slice(b"TXT");
        dir_block[11] = 0x20; // Archive attribute
        LittleEndian::write_u32(&mut dir_block[28..32], 4096); // 4096 bytes

        let files = fat32::parse_fat32_entries(&dir_block);
        assert_eq!(files.len(), 1);
        assert!(files[0].is_deleted);
        assert_eq!(files[0].size_bytes, 4096);
    }

    #[test]
    fn test_ntfs_mft_record_parsing() {
        let mut mft = vec![0u8; 1024];
        mft[0..4].copy_from_slice(b"FILE");
        LittleEndian::write_u16(&mut mft[0x14..0x16], 56); // First attr at 56
        LittleEndian::write_u16(&mut mft[0x16..0x18], 0x00); // 0x00 = Deleted file!

        // Attribute $FILE_NAME (0x30)
        let attr_start = 56;
        LittleEndian::write_u32(&mut mft[attr_start..attr_start + 4], 0x30);
        LittleEndian::write_u32(&mut mft[attr_start + 4..attr_start + 8], 96);
        mft[attr_start + 8] = 0; // Resident
        LittleEndian::write_u16(&mut mft[attr_start + 20..attr_start + 22], 24); // Offset to body

        let body = attr_start + 24;
        mft[body + 64] = 8; // 8 chars
        let name_utf16 = "test.txt".encode_utf16().collect::<Vec<_>>();
        for (i, &c) in name_utf16.iter().enumerate() {
            LittleEndian::write_u16(&mut mft[body + 66 + i * 2..body + 66 + (i + 1) * 2], c);
        }

        let parsed = ntfs::parse_mft_record(&mft).expect("Should parse deleted NTFS MFT record");
        assert_eq!(parsed.filename, "test.txt");
        assert!(parsed.is_deleted);
    }
}
