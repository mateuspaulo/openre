use chrono::Utc;
use recovery_core::{RecoveredFile, RecoveryError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub session_id: String,
    pub timestamp: String,
    pub source_id: String,
    pub destination_path: String,
    pub total_files_discovered: usize,
    pub total_files_recovered: usize,
    pub total_bytes_recovered: u64,
    pub duration_seconds: f64,
    pub files: Vec<RecoveredFile>,
    pub errors: Vec<String>,
}

impl RecoveryReport {
    pub fn new(
        session_id: String,
        source_id: String,
        destination_path: String,
        duration_seconds: f64,
        files: Vec<RecoveredFile>,
        errors: Vec<String>,
    ) -> Self {
        let total_files_discovered = files.len();
        let total_files_recovered = files.iter().filter(|f| f.size_bytes > 0).count();
        let total_bytes_recovered = files.iter().map(|f| f.size_bytes).sum();

        Self {
            session_id,
            timestamp: Utc::now().to_rfc3339(),
            source_id,
            destination_path,
            total_files_discovered,
            total_files_recovered,
            total_bytes_recovered,
            duration_seconds,
            files,
            errors,
        }
    }

    pub fn to_json(&self) -> Result<String, RecoveryError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# OpenRe — Recovery Audit Report\n\n");
        md.push_str(&format!("- **Session ID:** `{}`\n", self.session_id));
        md.push_str(&format!("- **Generated At:** {}\n", self.timestamp));
        md.push_str(&format!("- **Source Media:** `{}`\n", self.source_id));
        md.push_str(&format!("- **Destination Directory:** `{}`\n", self.destination_path));
        md.push_str(&format!("- **Duration:** {:.2} seconds\n", self.duration_seconds));
        md.push_str(&format!("- **Files Discovered:** {}\n", self.total_files_discovered));
        md.push_str(&format!("- **Files Successfully Recovered:** {}\n", self.total_files_recovered));
        md.push_str(&format!(
            "- **Total Data Recovered:** {:.2} MB ({} bytes)\n\n",
            self.total_bytes_recovered as f64 / (1024.0 * 1024.0),
            self.total_bytes_recovered
        ));

        md.push_str("## Recovered Files Inventory\n\n");
        md.push_str("| Filename | Category | Size (Bytes) | Integrity Rating | SHA-256 Hash |\n");
        md.push_str("| :--- | :---: | :---: | :---: | :--- |\n");

        for file in &self.files {
            let hash_str = file.sha256.as_deref().unwrap_or("N/A");
            md.push_str(&format!(
                "| `{}` | {:?} | {} | {} | `{}` |\n",
                file.filename,
                file.category,
                file.size_bytes,
                file.integrity.as_str(),
                hash_str
            ));
        }

        if !self.errors.is_empty() {
            md.push_str("\n## Errors & Warnings\n\n");
            for err in &self.errors {
                md.push_str(&format!("- ⚠️ {}\n", err));
            }
        }

        md.push_str("\n---\n*Report generated strictly locally by OpenRe. Zero telemetry.* \n");
        md
    }

    pub fn to_csv(&self) -> String {
        let mut csv = String::new();
        csv.push_str("ID,Filename,Category,Extension,SizeBytes,SourceOffset,Integrity,SHA256\n");
        for f in &self.files {
            let hash_str = f.sha256.as_deref().unwrap_or("");
            csv.push_str(&format!(
                "\"{}\",\"{}\",\"{:?}\",\"{}\",{},{},\"{}\",\"{}\"\n",
                f.id,
                f.filename,
                f.category,
                f.extension,
                f.size_bytes,
                f.source_offset,
                f.integrity.as_str(),
                hash_str
            ));
        }
        csv
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recovery_core::{FileCategory, IntegrityStatus, MetadataSource};

    #[test]
    fn test_report_generation() {
        let files = vec![RecoveredFile {
            id: "1".to_string(),
            filename: "photo.jpg".to_string(),
            original_path: None,
            category: FileCategory::Image,
            extension: "jpg".to_string(),
            size_bytes: 1048576,
            source_offset: 512,
            integrity: IntegrityStatus::LikelyRecoverable,
            created_at: None,
            modified_at: None,
            metadata_source: MetadataSource::SignatureCarving,
            sha256: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
        }];

        let report = RecoveryReport::new(
            "sess_001".to_string(),
            "\\\\.\\PhysicalDrive1".to_string(),
            "D:\\Recovered".to_string(),
            4.5,
            files,
            vec![],
        );

        let json = report.to_json().unwrap();
        assert!(json.contains("photo.jpg"));

        let md = report.to_markdown();
        assert!(md.contains("# OpenRe — Recovery Audit Report"));
        assert!(md.contains("Likely recoverable"));

        let csv = report.to_csv();
        assert!(csv.contains("photo.jpg"));
    }
}
