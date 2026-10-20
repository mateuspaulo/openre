use byteorder::{ByteOrder, LittleEndian};
use device_access::BlockReader;
use file_carving::carve_at_offset;
use filesystem::{detect_filesystem, exfat, fat32, ntfs};
use recovery_core::{
    check_source_dest_collision, safe_resolve_output_path, sanitize_file_name,
    CancellationToken, FileCategory, IntegrityStatus, MetadataSource, RecoveredFile,
    RecoveryError, ScanProgress,
};
use reporting::RecoveryReport;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Quick,
    Deep,
}

pub struct Scanner;

impl Scanner {
    /// Executes Quick Scan: First attempts filesystem metadata parsing (NTFS/FAT32/exFAT),
    /// and falls back to folder hierarchy inspection if source is a directory.
    pub fn quick_scan<F>(
        reader: &mut dyn BlockReader,
        cancel: &CancellationToken,
        mut on_progress: F,
    ) -> Result<Vec<RecoveredFile>, RecoveryError>
    where
        F: FnMut(ScanProgress),
    {
        let mut results = Vec::new();
        let total = reader.total_bytes();
        let mut progress = ScanProgress {
            bytes_scanned: 0,
            total_bytes: total,
            files_found: 0,
            read_errors: 0,
            speed_bytes_per_sec: 0,
            current_label: "Checking filesystem structures...".to_string(),
            is_complete: false,
        };
        on_progress(progress.clone());

        let mut sector_0 = [0u8; 4096];
        let bytes_read = reader.read_at(0, &mut sector_0).unwrap_or(0);

        if bytes_read >= 512 {
            if let Some(fs_type) = detect_filesystem(&sector_0) {
                progress.current_label = format!("Detected {fs_type} filesystem. Reading metadata...");
                on_progress(progress.clone());

                let mut last_emit = Instant::now();

                match fs_type {
                    "NTFS" => {
                        let bytes_per_sector = LittleEndian::read_u16(&sector_0[0x0B..0x0D]) as u64;
                        let sectors_per_cluster = sector_0[0x0D] as u64;
                        let cluster_size = if bytes_per_sector >= 512 && sectors_per_cluster > 0 {
                            bytes_per_sector * sectors_per_cluster
                        } else {
                            4096
                        };
                        let mft_lcn = LittleEndian::read_u64(&sector_0[0x30..0x38]);
                        let mft_start = mft_lcn * cluster_size;

                        let mut scan_ranges = Vec::new();
                        if mft_start > 0 && mft_start < total {
                            scan_ranges.push((mft_start, (mft_start + 40 * 1024 * 1024).min(total)));
                        }
                        scan_ranges.push((0, total.min(20 * 1024 * 1024)));

                        let mut mft_buf = vec![0u8; 1024];

                        for (start_off, limit_off) in scan_ranges {
                            let mut offset = start_off;
                            while offset + 1024 <= limit_off {
                                if cancel.is_cancelled() {
                                    break;
                                }

                                if reader.read_at(offset, &mut mft_buf).is_ok() {
                                    if let Some(mft_file) = ntfs::parse_mft_record(&mft_buf) {
                                        let id = format!("ntfs_{offset:08X}");
                                        results.push(mft_file.to_recovered_file(id));
                                        progress.files_found = results.len();
                                    }
                                }
                                offset += 1024;
                                progress.bytes_scanned = offset;
                                if last_emit.elapsed().as_millis() >= 100 {
                                    last_emit = Instant::now();
                                    on_progress(progress.clone());
                                }
                            }
                            if !results.is_empty() || cancel.is_cancelled() {
                                break;
                            }
                        }
                    }
                    "FAT32" => {
                        let scan_limit = total.min(30 * 1024 * 1024);
                        let mut dir_buf = vec![0u8; 4096];
                        let mut offset = 0u64;

                        while offset + 4096 <= scan_limit {
                            if cancel.is_cancelled() {
                                break;
                            }

                            if reader.read_at(offset, &mut dir_buf).is_ok() {
                                let fat_files = fat32::parse_fat32_entries(&dir_buf);
                                for ff in fat_files {
                                    let id = format!("fat32_{:08X}", ff.cluster_or_sector_offset);
                                    results.push(ff.to_recovered_file(id));
                                    progress.files_found = results.len();
                                }
                            }
                            offset += 4096;
                            progress.bytes_scanned = offset;
                            if last_emit.elapsed().as_millis() >= 100 {
                                last_emit = Instant::now();
                                on_progress(progress.clone());
                            }
                        }
                    }
                    "exFAT" => {
                        let scan_limit = total.min(30 * 1024 * 1024);
                        let mut dir_buf = vec![0u8; 4096];
                        let mut offset = 0u64;

                        while offset + 4096 <= scan_limit {
                            if cancel.is_cancelled() {
                                break;
                            }

                            if reader.read_at(offset, &mut dir_buf).is_ok() {
                                let exfat_files = exfat::parse_exfat_entries(&dir_buf);
                                for ef in exfat_files {
                                    let id = format!("exfat_{:08X}", ef.cluster_or_sector_offset);
                                    results.push(ef.to_recovered_file(id));
                                    progress.files_found = results.len();
                                }
                            }
                            offset += 4096;
                            progress.bytes_scanned = offset;
                            if last_emit.elapsed().as_millis() >= 100 {
                                last_emit = Instant::now();
                                on_progress(progress.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // If no deleted filesystem records were found (e.g. source is a user folder path or volume without admin)
        let device_id = reader.device_id();
        let source_path_str = if device_id.len() == 2 && device_id.ends_with(':') {
            format!("{}:\\", device_id.chars().next().unwrap())
        } else {
            device_id.to_string()
        };
        let source_path = Path::new(&source_path_str);
        if results.is_empty() && (source_path.is_dir() || Path::new(device_id).is_dir()) {
            let active_dir = if source_path.is_dir() { source_path } else { Path::new(device_id) };
            progress.current_label = format!("Scanning directory tree at {}...", active_dir.display());
            on_progress(progress.clone());

            let mut last_emit = Instant::now();

            for entry in WalkDir::new(active_dir).into_iter().flatten() {
                if cancel.is_cancelled() {
                    break;
                }

                if entry.file_type().is_file() {
                    let file_path = entry.path();
                    let filename = file_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".to_string());
                    let ext = file_path
                        .extension()
                        .map(|e| e.to_string_lossy().to_string())
                        .unwrap_or_default()
                        .to_lowercase();
                    let metadata = entry.metadata().ok();
                    let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

                    let category = FileCategory::from_extension(&ext);
                    let id = format!("dir_{}", results.len());

                    results.push(RecoveredFile {
                        id,
                        filename,
                        original_path: Some(file_path.to_string_lossy().to_string()),
                        category,
                        extension: ext,
                        size_bytes,
                        source_offset: 0,
                        integrity: IntegrityStatus::LikelyRecoverable,
                        created_at: None,
                        modified_at: None,
                        metadata_source: MetadataSource::FilesystemNtfs,
                        sha256: None,
                    });

                    progress.files_found = results.len();
                    progress.bytes_scanned += size_bytes;

                    if last_emit.elapsed().as_millis() >= 100 {
                        last_emit = Instant::now();
                        on_progress(progress.clone());
                    }
                }
            }
        }

        progress.is_complete = true;
        if cancel.is_cancelled() {
            progress.current_label = format!("Quick scan stopped. Preserved {} files found so far.", results.len());
        } else {
            progress.current_label = format!("Quick scan complete. Found {} files.", results.len());
        }
        on_progress(progress);

        Ok(results)
    }

    /// Executes Deep Scan: High-throughput block-strided carving across entire media
    pub fn deep_scan<F>(
        reader: &mut dyn BlockReader,
        cancel: &CancellationToken,
        mut on_progress: F,
    ) -> Result<Vec<RecoveredFile>, RecoveryError>
    where
        F: FnMut(ScanProgress),
    {
        let mut results = Vec::new();
        let total = reader.total_bytes();
        let start_time = Instant::now();

        let window_size = 1024 * 1024usize; // 1 MB sliding chunk window
        let stride = 512usize; // Standard sector alignment

        let mut window = vec![0u8; window_size];
        let mut offset = 0u64;

        let mut progress = ScanProgress {
            bytes_scanned: 0,
            total_bytes: total,
            files_found: 0,
            read_errors: 0,
            speed_bytes_per_sec: 0,
            current_label: "Starting deep block carving...".to_string(),
            is_complete: false,
        };
        on_progress(progress.clone());

        while offset < total {
            if cancel.is_cancelled() {
                break;
            }

            let to_read = (window_size as u64).min(total - offset) as usize;
            match reader.read_at(offset, &mut window[..to_read]) {
                Ok(bytes_read) => {
                    if bytes_read < 16 {
                        if offset + window_size as u64 >= total {
                            break;
                        } else {
                            offset += to_read.max(stride) as u64;
                            continue;
                        }
                    }

                    let slice = &window[..bytes_read];
                    let mut local_pos = 0usize;

                    while local_pos + 16 <= slice.len() {
                        if cancel.is_cancelled() {
                            break;
                        }

                        if let Some(carve) = carve_at_offset(&slice[local_pos..]) {
                            let file_offset = offset + local_pos as u64;
                            let filename = format!("carved_{file_offset:08X}.{}", carve.extension);
                            let id = format!("carve_{file_offset:08X}");

                            results.push(RecoveredFile {
                                id,
                                filename,
                                original_path: None,
                                category: carve.category,
                                extension: carve.extension.to_string(),
                                size_bytes: carve.length_bytes,
                                source_offset: file_offset,
                                integrity: carve.integrity,
                                created_at: None,
                                modified_at: None,
                                metadata_source: MetadataSource::SignatureCarving,
                                sha256: None,
                            });

                            progress.files_found = results.len();
                            // Advance past detected file or at least one stride
                            local_pos += (carve.length_bytes as usize).max(stride);
                        } else {
                            local_pos += stride;
                        }
                    }

                    offset += bytes_read as u64;
                }
                Err(_) => {
                    progress.read_errors += 1;
                    offset += stride as u64;
                }
            }

            let elapsed = start_time.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                progress.speed_bytes_per_sec = (offset as f64 / elapsed) as u64;
            }
            progress.bytes_scanned = offset;
            progress.current_label = format!(
                "Scanning: {:.1}% ({:.1} MB/s)",
                if total > 0 { (offset as f64 / total as f64) * 100.0 } else { 0.0 },
                progress.speed_bytes_per_sec as f64 / (1024.0 * 1024.0)
            );
            on_progress(progress.clone());
        }

        progress.is_complete = true;
        if cancel.is_cancelled() {
            progress.current_label = format!("Deep scan stopped by user. Preserved {} files found so far.", results.len());
        } else {
            progress.current_label = format!("Deep scan finished. Salvaged {} files.", results.len());
        }
        on_progress(progress);

        Ok(results)
    }

    /// Safely writes recovered files to the destination directory.
    pub fn recover_files<F>(
        reader: &mut dyn BlockReader,
        files: &[RecoveredFile],
        dest_dir: &Path,
        cancel: &CancellationToken,
        mut on_file_recovered: F,
    ) -> Result<RecoveryReport, RecoveryError>
    where
        F: FnMut(&RecoveredFile, usize, usize),
    {
        check_source_dest_collision(reader.device_id(), dest_dir)?;
        if !dest_dir.exists() {
            std::fs::create_dir_all(dest_dir)?;
        }

        let start_time = Instant::now();
        let mut final_files = Vec::new();
        let mut errors = Vec::new();

        let mut read_buf = vec![0u8; 64 * 1024];

        for (idx, file) in files.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(RecoveryError::Cancelled);
            }

            let safe_name = sanitize_file_name(&file.filename);
            let mut target_path = safe_resolve_output_path(dest_dir, &safe_name)?;

            // Prevent overwriting existing files by appending counter
            let mut counter = 1;
            while target_path.exists() {
                let stem = Path::new(&safe_name)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                let new_name = format!("{stem}_{counter}.{}", file.extension);
                target_path = safe_resolve_output_path(dest_dir, &new_name)?;
                counter += 1;
            }

            let mut out_file = match File::create(&target_path) {
                Ok(f) => f,
                Err(e) => {
                    errors.push(format!("Failed to create output file {:?}: {}", target_path, e));
                    continue;
                }
            };

            let mut hasher = Sha256::new();
            let mut bytes_written = 0u64;
            let total_to_recover = file.size_bytes;

            // If file has an existing original path on disk, copy from it directly
            if let Some(ref orig) = file.original_path {
                if Path::new(orig).is_file() {
                    if let Ok(mut src_f) = File::open(orig) {
                        use std::io::Read;
                        while bytes_written < total_to_recover {
                            let to_read = (read_buf.len() as u64).min(total_to_recover - bytes_written) as usize;
                            match src_f.read(&mut read_buf[..to_read]) {
                                Ok(n) if n > 0 => {
                                    let chunk = &read_buf[..n];
                                    if let Err(e) = out_file.write_all(chunk) {
                                        errors.push(format!("Write error on {:?}: {}", target_path, e));
                                        break;
                                    }
                                    hasher.update(chunk);
                                    bytes_written += n as u64;
                                }
                                _ => break,
                            }
                        }
                    }
                }
            }

            // Fallback to reading from block reader if not recovered via original_path
            if bytes_written == 0 && total_to_recover > 0 {
                let mut current_offset = file.source_offset;
                while bytes_written < total_to_recover {
                    let to_read = (read_buf.len() as u64).min(total_to_recover - bytes_written) as usize;
                    match reader.read_at(current_offset, &mut read_buf[..to_read]) {
                        Ok(n) if n > 0 => {
                            let chunk = &read_buf[..n];
                            if let Err(e) = out_file.write_all(chunk) {
                                errors.push(format!("Write error on {:?}: {}", target_path, e));
                                break;
                            }
                            hasher.update(chunk);
                            bytes_written += n as u64;
                            current_offset += n as u64;
                        }
                        _ => {
                            errors.push(format!("Premature EOF reading {:?} at offset {}", file.filename, current_offset));
                            break;
                        }
                    }
                }
            }

            let sha256_hex = format!("{:x}", hasher.finalize());
            let mut updated_file = file.clone();
            updated_file.sha256 = Some(sha256_hex);
            final_files.push(updated_file.clone());

            on_file_recovered(&updated_file, idx + 1, files.len());
        }

        let duration = start_time.elapsed().as_secs_f64();
        let report = RecoveryReport::new(
            format!("session_{}", Instant::now().elapsed().as_micros()),
            reader.device_id().to_string(),
            dest_dir.to_string_lossy().to_string(),
            duration,
            final_files,
            errors,
        );

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use device_access::MemoryBlockReader;

    #[test]
    fn test_deep_scan_carves_sample() {
        let mut disk = vec![0u8; 4096];
        // Embed a JPEG at offset 512
        let jpeg = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0xFF, 0xD9,
        ];
        disk[512..512 + jpeg.len()].copy_from_slice(&jpeg);

        let mut reader = MemoryBlockReader::new("test://mem", disk);
        let cancel = CancellationToken::new();

        let files = Scanner::deep_scan(&mut reader, &cancel, |_p| {}).expect("Deep scan should run");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].extension, "jpg");
        assert_eq!(files[0].source_offset, 512);
        assert_eq!(files[0].integrity, IntegrityStatus::LikelyRecoverable);
    }
}
