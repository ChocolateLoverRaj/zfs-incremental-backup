use serde::{Deserialize, Serialize};

use crate::diff_or_first::DiffEntry;

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupState {
    pub diff: Vec<DiffEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupData {
    pub s3_bucket: String,
    /// Idk if we will use this but it would be useful in case the region chances in the local AWS credentials / config file
    pub s3_region: String,
    pub last_saved_snapshot_name: Option<String>,
    pub backup_state: Option<BackupState>,
}
