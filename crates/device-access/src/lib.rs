use recovery_core::{
    check_source_dest_collision, CancellationToken, DeviceInfo, DeviceType, RecoveryError,
};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub trait BlockReader: Send {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, RecoveryError>;
    fn total_bytes(&self) -> u64;
    fn device_id(&self) -> &str;
}

pub struct FileBlockReader {
    id: String,
    file: File,
    size: u64,
}

impl FileBlockReader {
    pub fn open(path: &Path) -> Result<Self, RecoveryError> {
        let file = OpenOptions::new()
            .read(true)
            .write(false) // Strictly read-only
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    RecoveryError::PermissionDenied(format!("Permission denied opening {:?}", path))
                } else {
                    RecoveryError::Io(e)
                }
            })?;

        let size = file.metadata()?.len();
        Ok(Self {
            id: path.to_string_lossy().to_string(),
            file,
            size,
        })
    }
}

impl BlockReader for FileBlockReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, RecoveryError> {
        if offset >= self.size {
            return Ok(0);
        }
        self.file.seek(SeekFrom::Start(offset))?;
        let to_read = (buf.len() as u64).min(self.size - offset) as usize;
        let read_bytes = self.file.read(&mut buf[..to_read])?;
        Ok(read_bytes)
    }

    fn total_bytes(&self) -> u64 {
        self.size
    }

    fn device_id(&self) -> &str {
        &self.id
    }
}

pub struct MemoryBlockReader {
    id: String,
    data: Vec<u8>,
}

impl MemoryBlockReader {
    pub fn new(id: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            id: id.into(),
            data,
        }
    }
}

impl BlockReader for MemoryBlockReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, RecoveryError> {
        let start = offset as usize;
        if start >= self.data.len() {
            return Ok(0);
        }
        let end = (start + buf.len()).min(self.data.len());
        let count = end - start;
        buf[..count].copy_from_slice(&self.data[start..end]);
        Ok(count)
    }

    fn total_bytes(&self) -> u64 {
        self.data.len() as u64
    }

    fn device_id(&self) -> &str {
        &self.id
    }
}

/// Enumerate available drives, volumes, and removable media
pub fn enumerate_devices() -> Vec<DeviceInfo> {
    let mut devices = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // Enumerate Windows logical drive letters
        for letter in b'A'..=b'Z' {
            let drive_path = format!("{}:\\", letter as char);
            let path = Path::new(&drive_path);
            if path.exists() {
                let size = get_disk_free_space_windows(&drive_path).unwrap_or(0);
                devices.push(DeviceInfo {
                    id: format!("{}:", letter as char),
                    name: format!("Local Disk ({}:)", letter as char),
                    device_type: DeviceType::Volume,
                    mount_point: Some(drive_path),
                    size_bytes: size,
                    filesystem: None,
                    is_read_only: false,
                    requires_elevation: false,
                });
            }
        }

        // Add standard Physical Drives (requires elevation for direct sector IO)
        for i in 0..8 {
            let id = format!("\\\\.\\PhysicalDrive{}", i);
            devices.push(DeviceInfo {
                id: id.clone(),
                name: format!("Physical Drive {}", i),
                device_type: DeviceType::PhysicalDrive,
                mount_point: None,
                size_bytes: 0,
                filesystem: None,
                is_read_only: false,
                requires_elevation: true,
            });
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Enumerate common Linux block devices
        if let Ok(entries) = std::fs::read_dir("/sys/block") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("sd") || name.starts_with("nvme") || name.starts_with("mmcblk") {
                    let dev_path = format!("/dev/{}", name);
                    devices.push(DeviceInfo {
                        id: dev_path.clone(),
                        name: format!("Block Device ({})", name),
                        device_type: DeviceType::PhysicalDrive,
                        mount_point: None,
                        size_bytes: 0,
                        filesystem: None,
                        is_read_only: false,
                        requires_elevation: true,
                    });
                }
            }
        }
    }

    devices
}

#[cfg(target_os = "windows")]
fn get_disk_free_space_windows(drive_root: &str) -> Option<u64> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }

    let wide: Vec<u16> = OsStr::new(drive_root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_avail = 0u64;
    let mut total_bytes = 0u64;
    let mut total_free = 0u64;

    let res = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_avail,
            &mut total_bytes,
            &mut total_free,
        )
    };

    if res != 0 {
        Some(total_bytes)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct DiskImagingReport {
    pub total_sectors: u64,
    pub copied_bytes: u64,
    pub bad_sectors_count: u64,
    pub unreadable_ranges: Vec<(u64, u64)>,
}

/// Clones a source BlockReader into a raw disk image file (.img/.dd) with bad sector tracking.
pub fn create_disk_image<F>(
    source: &mut dyn BlockReader,
    dest_path: &Path,
    cancel: &CancellationToken,
    mut on_progress: F,
) -> Result<DiskImagingReport, RecoveryError>
where
    F: FnMut(u64, u64),
{
    check_source_dest_collision(source.device_id(), dest_path)?;

    let total = source.total_bytes();
    let sector_size = 512usize;
    let chunk_size = 64 * 1024usize; // 64KB read batches

    let mut dest_file = File::create(dest_path)?;
    let mut offset = 0u64;
    let mut buf = vec![0u8; chunk_size];
    let mut bad_sectors = 0u64;
    let mut unreadable_ranges = Vec::new();

    while offset < total {
        if cancel.is_cancelled() {
            return Err(RecoveryError::Cancelled);
        }

        let to_read = (chunk_size as u64).min(total - offset) as usize;
        match source.read_at(offset, &mut buf[..to_read]) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                dest_file.write_all(&buf[..bytes_read])?;
                offset += bytes_read as u64;
            }
            Err(_) => {
                // If 64KB chunk failed, step down to 512-byte single sector retry
                let mut sector_buf = [0u8; 512];
                let mut sector_offset = offset;
                let chunk_end = (offset + to_read as u64).min(total);

                while sector_offset < chunk_end {
                    if cancel.is_cancelled() {
                        return Err(RecoveryError::Cancelled);
                    }
                    match source.read_at(sector_offset, &mut sector_buf) {
                        Ok(b) => {
                            dest_file.write_all(&sector_buf[..b])?;
                        }
                        Err(_) => {
                            // Write zeros for unreadable sector to preserve offset alignment
                            dest_file.write_all(&[0u8; 512])?;
                            bad_sectors += 1;
                            unreadable_ranges.push((sector_offset, sector_offset + 512));
                        }
                    }
                    sector_offset += sector_size as u64;
                }
                offset = chunk_end;
            }
        }

        on_progress(offset, total);
    }

    Ok(DiskImagingReport {
        total_sectors: total / 512,
        copied_bytes: offset,
        bad_sectors_count: bad_sectors,
        unreadable_ranges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_block_reader() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut reader = MemoryBlockReader::new("mem://test", data);
        assert_eq!(reader.total_bytes(), 10);

        let mut buf = [0u8; 4];
        let n = reader.read_at(2, &mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(buf, [3, 4, 5, 6]);
    }

    #[test]
    fn test_disk_imaging_simulation() {
        let fake_disk = vec![0xAA; 16384]; // 16 KB fake disk
        let mut reader = MemoryBlockReader::new("fake://disk", fake_disk);

        let temp_dir = std::env::temp_dir();
        let image_path = temp_dir.join("test_openre_imaging.img");

        let cancel = CancellationToken::new();
        let report = create_disk_image(&mut reader, &image_path, &cancel, |_scanned, _total| {})
            .expect("Imaging should succeed");

        assert_eq!(report.copied_bytes, 16384);
        assert_eq!(report.bad_sectors_count, 0);

        let readback = std::fs::read(&image_path).unwrap();
        assert_eq!(readback.len(), 16384);
        assert_eq!(readback[0], 0xAA);

        let _ = std::fs::remove_file(image_path);
    }
}
