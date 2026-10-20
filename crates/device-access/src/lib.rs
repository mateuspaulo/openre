use recovery_core::{
    check_source_dest_collision, CancellationToken, DeviceInfo, DeviceType, RecoveryError,
};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

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

pub struct FolderSpan {
    pub path: PathBuf,
    pub start_offset: u64,
    pub size: u64,
}

pub struct FolderBlockReader {
    id: String,
    spans: Vec<FolderSpan>,
    total_size: u64,
}

impl FolderBlockReader {
    pub fn open(dir_path: &Path) -> Result<Self, RecoveryError> {
        let mut spans = Vec::new();
        let mut current_offset = 0u64;

        for entry in walkdir::WalkDir::new(dir_path).into_iter().flatten() {
            if entry.file_type().is_file() {
                if let Ok(meta) = entry.metadata() {
                    let sz = meta.len();
                    if sz > 0 {
                        spans.push(FolderSpan {
                            path: entry.path().to_path_buf(),
                            start_offset: current_offset,
                            size: sz,
                        });
                        current_offset += sz;
                    }
                }
            }
        }

        Ok(Self {
            id: dir_path.to_string_lossy().to_string(),
            spans,
            total_size: current_offset,
        })
    }
}

impl BlockReader for FolderBlockReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, RecoveryError> {
        if offset >= self.total_size || buf.is_empty() {
            return Ok(0);
        }

        let mut total_written = 0usize;
        let mut cur_offset = offset;

        for span in &self.spans {
            if total_written >= buf.len() {
                break;
            }
            let span_end = span.start_offset + span.size;
            if cur_offset >= span.start_offset && cur_offset < span_end {
                let offset_in_file = cur_offset - span.start_offset;
                let available_in_span = span.size - offset_in_file;
                let needed = (buf.len() - total_written) as u64;
                let to_read = needed.min(available_in_span) as usize;

                if let Ok(mut f) = File::open(&span.path) {
                    if f.seek(SeekFrom::Start(offset_in_file)).is_ok() {
                        if let Ok(n) = f.read(&mut buf[total_written..total_written + to_read]) {
                            total_written += n;
                            cur_offset += n as u64;
                        }
                    }
                }
            }
        }

        Ok(total_written)
    }

    fn total_bytes(&self) -> u64 {
        self.total_size
    }

    fn device_id(&self) -> &str {
        &self.id
    }
}

#[cfg(target_os = "windows")]
pub struct VolumeBlockReader {
    id: String,
    file: File,
    size: u64,
}

#[cfg(target_os = "windows")]
impl VolumeBlockReader {
    pub fn open_volume(drive_letter: char) -> Result<Self, RecoveryError> {
        let root = format!("{}:\\", drive_letter.to_ascii_uppercase());
        let device_path = format!(r"\\.\{}:", drive_letter.to_ascii_uppercase());

        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .open(&device_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    RecoveryError::PermissionDenied(format!(
                        "Administrator privileges required to access raw volume {}. Please launch OpenRe with 'Run as Administrator'.",
                        device_path
                    ))
                } else {
                    RecoveryError::Io(e)
                }
            })?;

        let size = get_disk_free_space_windows(&root).unwrap_or(0);

        Ok(Self {
            id: format!("{}:", drive_letter.to_ascii_uppercase()),
            file,
            size,
        })
    }

    pub fn open_physical_drive(device_path: &str) -> Result<Self, RecoveryError> {
        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .open(device_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    RecoveryError::PermissionDenied(format!(
                        "Administrator privileges required to access physical device {}. Please launch OpenRe with 'Run as Administrator'.",
                        device_path
                    ))
                } else {
                    RecoveryError::Io(e)
                }
            })?;

        let size = query_physical_drive_size(&file).unwrap_or(0);

        Ok(Self {
            id: device_path.to_string(),
            file,
            size,
        })
    }
}

#[cfg(target_os = "windows")]
impl BlockReader for VolumeBlockReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, RecoveryError> {
        if self.size > 0 && offset >= self.size {
            return Ok(0);
        }
        if buf.is_empty() {
            return Ok(0);
        }

        // Align to 4096-byte boundaries for direct disk sector reading
        const SECTOR_SIZE: u64 = 4096;
        let aligned_start = (offset / SECTOR_SIZE) * SECTOR_SIZE;
        let aligned_end = ((offset + buf.len() as u64 + SECTOR_SIZE - 1) / SECTOR_SIZE) * SECTOR_SIZE;
        let aligned_len = (aligned_end - aligned_start) as usize;

        let mut scratch = vec![0u8; aligned_len];
        self.file.seek(SeekFrom::Start(aligned_start))?;

        let bytes_read = self.file.read(&mut scratch)?;

        let start_within_scratch = (offset - aligned_start) as usize;
        if start_within_scratch >= bytes_read {
            return Ok(0);
        }

        let available = bytes_read - start_within_scratch;
        let to_copy = buf.len().min(available);
        buf[..to_copy].copy_from_slice(&scratch[start_within_scratch..start_within_scratch + to_copy]);
        Ok(to_copy)
    }

    fn total_bytes(&self) -> u64 {
        self.size
    }

    fn device_id(&self) -> &str {
        &self.id
    }
}

#[cfg(target_os = "windows")]
fn query_physical_drive_size(file: &File) -> Option<u64> {
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    extern "system" {
        fn DeviceIoControl(
            hDevice: *mut std::ffi::c_void,
            dwIoControlCode: u32,
            lpInBuffer: *const std::ffi::c_void,
            nInBufferSize: u32,
            lpOutBuffer: *mut std::ffi::c_void,
            nOutBufferSize: u32,
            lpBytesReturned: *mut u32,
            lpOverlapped: *mut std::ffi::c_void,
        ) -> i32;
    }

    const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x00074059;
    let handle = file.as_raw_handle();
    let mut length_info: i64 = 0;
    let mut bytes_returned: u32 = 0;

    let res = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_GET_LENGTH_INFO,
            std::ptr::null(),
            0,
            &mut length_info as *mut i64 as *mut std::ffi::c_void,
            std::mem::size_of::<i64>() as u32,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if res != 0 && length_info > 0 {
        Some(length_info as u64)
    } else {
        None
    }
}

/// Opens any storage source (file, directory, raw Windows volume, physical drive)
/// returning a unified Box<dyn BlockReader>.
pub fn open_block_reader(source_id: &str) -> Result<Box<dyn BlockReader>, RecoveryError> {
    let path = Path::new(source_id);

    // 1. If it's a directory on disk, open as FolderBlockReader
    if path.is_dir() {
        return Ok(Box::new(FolderBlockReader::open(path)?));
    }

    // 2. If it's a regular file or raw disk image (.img, .dd, etc.)
    if path.is_file() {
        return Ok(Box::new(FileBlockReader::open(path)?));
    }

    // 3. Windows logical drive letters (e.g. "C:", "C:\", "D:", "d:")
    #[cfg(target_os = "windows")]
    {
        let trimmed = source_id.trim();
        let is_drive_letter = if trimmed.len() == 2 && trimmed.ends_with(':') {
            trimmed.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
        } else if trimmed.len() == 3 && trimmed.ends_with(":\\") {
            trimmed.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
        } else {
            false
        };

        if is_drive_letter {
            let letter = trimmed.chars().next().unwrap();
            match VolumeBlockReader::open_volume(letter) {
                Ok(reader) => return Ok(Box::new(reader)),
                Err(RecoveryError::PermissionDenied(msg)) => {
                    return Err(RecoveryError::PermissionDenied(msg));
                }
                Err(e) => {
                    // If opening raw volume failed, check if directory root exists as fallback
                    let root_str = format!("{}:\\", letter);
                    let root_path = Path::new(&root_str);
                    if root_path.is_dir() {
                        return Ok(Box::new(FolderBlockReader::open(root_path)?));
                    }
                    return Err(e);
                }
            }
        }

        // 4. Windows PhysicalDrive handles (e.g. "\\.\PhysicalDrive0")
        if trimmed.starts_with(r"\\.\") {
            return Ok(Box::new(VolumeBlockReader::open_physical_drive(trimmed)?));
        }
    }

    // 5. Fallback: try FileBlockReader
    Ok(Box::new(FileBlockReader::open(path)?))
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

    #[test]
    fn test_folder_block_reader() {
        let temp_dir = std::env::temp_dir().join("test_openre_folder_reader");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let f1 = temp_dir.join("file1.bin");
        let f2 = temp_dir.join("file2.bin");
        std::fs::write(&f1, b"HELLO_").unwrap();
        std::fs::write(&f2, b"WORLD!").unwrap();

        let mut folder_reader = FolderBlockReader::open(&temp_dir).expect("Should open folder");
        assert_eq!(folder_reader.total_bytes(), 12);

        let mut buf = [0u8; 12];
        let n = folder_reader.read_at(0, &mut buf).expect("Read should succeed");
        assert_eq!(n, 12);
        assert_eq!(&buf, b"HELLO_WORLD!");

        // Read with offset
        let mut part = [0u8; 5];
        let n_part = folder_reader.read_at(6, &mut part).expect("Partial read should succeed");
        assert_eq!(n_part, 5);
        assert_eq!(&part, b"WORLD");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_open_block_reader_on_directory() {
        let temp_dir = std::env::temp_dir().join("test_openre_dir_detect");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let f1 = temp_dir.join("sample.txt");
        std::fs::write(&f1, b"DATA123").unwrap();

        let reader_res = open_block_reader(&temp_dir.to_string_lossy());
        assert!(reader_res.is_ok(), "Directory should open as block reader");
        let reader = reader_res.unwrap();
        assert_eq!(reader.total_bytes(), 7);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
