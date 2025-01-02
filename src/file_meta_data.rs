use std::{fs::Metadata, time::SystemTime};

use serde::{Deserialize, Serialize};

/// Store times as options because it could fail / might not be available
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FileMetaData {
    pub accessed: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub len: u64,
}

impl From<&Metadata> for FileMetaData {
    fn from(value: &Metadata) -> Self {
        Self {
            accessed: value.accessed().ok(),
            created: value.created().ok(),
            modified: value.modified().ok(),
            len: value.len(),
        }
    }
}
