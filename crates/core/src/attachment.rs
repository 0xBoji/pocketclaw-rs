use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentPolicy {
    /// Whether to accept attachments at all.
    pub enabled: bool,
    /// Maximum size in bytes per file.
    pub max_size_bytes: usize,
    /// Allowed MIME types (e.g. "image/png").
    pub allowed_mime_types: Vec<String>,
    /// Where to store attachments relative to workspace.
    pub storage_directory: PathBuf,
}

impl Default for AttachmentPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_size_bytes: 10 * 1024 * 1024, // 10MB default
            allowed_mime_types: vec![
                "image/png".to_string(),
                "image/jpeg".to_string(),
                "image/webp".to_string(),
                "text/plain".to_string(),
                "application/pdf".to_string(),
            ],
            storage_directory: PathBuf::from("attachments"),
        }
    }
}
