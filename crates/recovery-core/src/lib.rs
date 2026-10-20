use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RecoveryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Device access denied. Elevated administrator/root privileges are required: {0}")]
    PermissionDenied(String),

    #[error("Device not found or disconnected: {0}")]
    DeviceNotFound(String),

    #[error("Invalid partition or filesystem structure: {0}")]
    InvalidStructure(String),

    #[error("Source and destination collision! Cannot recover data to the source drive: {0}")]
    SourceDestinationCollision(String),

    #[error("Path traversal detected! Attempted path outside allowed root: {0}")]
    PathTraversal(String),

    #[error("Operation cancelled by user")]
    Cancelled,

    #[error("Format parsing error: {0}")]
    FormatError(String),

    #[error("Repair failed: {0}")]
    RepairFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileCategory {
    Image,
    Document,
    Video,
    Audio,
    Archive,
    Text,
    Other,
}

impl FileCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileCategory::Image => "Image",
            FileCategory::Document => "Document",
            FileCategory::Video => "Video",
            FileCategory::Audio => "Audio",
            FileCategory::Archive => "Archive",
            FileCategory::Text => "Text",
            FileCategory::Other => "Other",
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "ico" | "raw" => {
                FileCategory::Image
            }
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" => {
                FileCategory::Document
            }
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" => FileCategory::Video,
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" => FileCategory::Audio,
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => FileCategory::Archive,
            "txt" | "md" | "json" | "xml" | "csv" | "log" | "rs" | "c" | "cpp" | "py" | "js" => {
                FileCategory::Text
            }
            _ => FileCategory::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityStatus {
    #[serde(rename = "Likely recoverable")]
    LikelyRecoverable,
    #[serde(rename = "Partially recoverable")]
    PartiallyRecoverable,
    #[serde(rename = "File structure detected; integrity unverified")]
    StructureDetectedUnverified,
    #[serde(rename = "Corrupted")]
    Corrupted,
    #[serde(rename = "Unknown")]
    Unknown,
}

impl IntegrityStatus {
    pub fn as_str(&self) -> &'static str {
        match self
 {
            IntegrityStatus::LikelyRecoverable => "Likely recoverable",
            IntegrityStatus::PartiallyRecoverable => "Partially recoverable",
            IntegrityStatus::StructureDetectedUnverified => {
                "File structure detected; integrity unverified"
            }
            IntegrityStatus::Corrupted => "Corrupted",
            IntegrityStatus::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataSource {
    FilesystemNtfs,
    FilesystemFat32,
    FilesystemExFat,
    FilesystemExt4,
    SignatureCarving,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveredFile {
    pub id: String,
    pub filename: String,
    pub original_path: Option<String>,
    pub category: FileCategory,
    pub extension: String,
    pub size_bytes: u64,
    pub source_offset: u64,
    pub integrity: IntegrityStatus,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub metadata_source: MetadataSource,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    PhysicalDrive,
    Volume,
    Partition,
    DiskImage,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub mount_point: Option<String>,
    pub size_bytes: u64,
    pub filesystem: Option<String>,
    pub is_read_only: bool,
    pub requires_elevation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub bytes_scanned: u64,
    pub total_bytes: u64,
    pub files_found: usize,
    pub read_errors: u64,
    pub speed_bytes_per_sec: u64,
    pub current_label: String,
    pub is_complete: bool,
}

impl Default for ScanProgress {
    fn default() -> Self {
        Self {
            bytes_scanned: 0,
            total_bytes: 0,
            files_found: 0,
            read_errors: 0,
            speed_bytes_per_sec: 0,
            current_label: String::from("Initializing..."),
            is_complete: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

pub fn sanitize_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other if other.is_control() => '_',
            other => other,
        })
        .collect();

    let trimmed = sanitized.trim_matches(&['.', ' '][..]);
    if trimmed.is_empty() {
        format!("recovered_file_{}", Utc::now().timestamp_micros())
    } else {
        trimmed.to_string()
    }
}

pub fn safe_resolve_output_path(base_dir: &Path, rel_path: &str) -> Result<PathBuf, RecoveryError> {
    let sanitized_segments: Vec<String> = rel_path
        .split(&['/', '\\'][..])
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .map(sanitize_file_name)
        .collect();

    let mut dest = base_dir.to_path_buf();
    for seg in sanitized_segments {
        dest.push(seg);
    }

    if !dest.starts_with(base_dir) {
        return Err(RecoveryError::PathTraversal(format!(
            "Path {:?} attempts to escape {:?}",
            dest, base_dir
        )));
    }

    Ok(dest)
}

pub fn check_source_dest_collision(source_id: &str, dest_path: &Path) -> Result<(), RecoveryError> {
    let dest_str = dest_path.to_string_lossy();
    let source_clean = source_id.trim();

    let is_volume_or_drive = (source_clean.len() == 2 && source_clean.ends_with(':'))
        || (source_clean.len() == 3 && source_clean.ends_with(":\\"))
        || source_clean.starts_with("\\\\.\\");

    if is_volume_or_drive && source_clean.len() >= 2 && source_clean.chars().nth(1) == Some(':') {
        let drive_letter = &source_clean[0..2];
        if dest_str.starts_with(drive_letter) {
            return Err(RecoveryError::SourceDestinationCollision(format!(
                "Destination directory is located on drive {drive_letter}, which is the source device! Writing to the source device risks destroying lost data."
            )));
        }
    }

    if let Ok(canon_dest) = dest_path.canonicalize() {
        if let Ok(canon_source) = Path::new(source_clean).canonicalize() {
            if canon_dest == canon_source {
                return Err(RecoveryError::SourceDestinationCollision(format!(
                    "Destination directory matches source image file {:?}",
                    canon_source
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_category_classification() {
        assert_eq!(FileCategory::from_extension("jpg"), FileCategory::Image);
        assert_eq!(FileCategory::from_extension("PNG"), FileCategory::Image);
        assert_eq!(FileCategory::from_extension("docx"), FileCategory::Document);
        assert_eq!(FileCategory::from_extension("pdf"), FileCategory::Document);
        assert_eq!(FileCategory::from_extension("mp4"), FileCategory::Video);
        assert_eq!(FileCategory::from_extension("mp3"), FileCategory::Audio);
        assert_eq!(FileCategory::from_extension("zip"), FileCategory::Archive);
        assert_eq!(FileCategory::from_extension("rs"), FileCategory::Text);
        assert_eq!(FileCategory::from_extension("unknownext"), FileCategory::Other);
    }

    #[test]
    fn test_sanitization() {
        let dirty = "my:bad*file/name?.png";
        let clean = sanitize_file_name(dirty);
        assert_eq!(clean, "my_bad_file_name_.png");
        assert!(sanitize_file_name("...").starts_with("recovered_file_"));
    }

    #[test]
    fn test_safe_path_resolution() {
        let base = Path::new("C:/safe/recovery");
        let malicious = "../../Windows/System32/calc.exe";
        let resolved = safe_resolve_output_path(base, malicious).unwrap();
        assert!(resolved.starts_with(base));
        assert!(!resolved.to_string_lossy().contains(".."));
    }

    #[test]
    fn test_collision_detection() {
        let err = check_source_dest_collision("D:\\", Path::new("D:\\RecoveredFiles"));
        assert!(err.is_err());
        let ok = check_source_dest_collision("D:\\", Path::new("E:\\RecoveredFiles"));
        assert!(ok.is_ok());
    }

    #[test]
    fn test_cancellation_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());
    }
}
