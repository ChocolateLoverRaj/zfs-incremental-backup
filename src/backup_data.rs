use argon2::password_hash::Salt;
use serde::{Deserialize, Serialize};

use crate::diff_or_first::DiffEntry;

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptionData {
    pub password_derived_key_salt: [u8; Salt::RECOMMENDED_LENGTH],
    pub encrypted_immutable_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupState {
    pub diff: Vec<DiffEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupData {
    pub s3_bucket: String,
    pub s3_region: String,
    pub encryption: Option<EncryptionData>,
    pub last_saved_snapshot_name: Option<String>,
    pub backup_state: Option<BackupState>,
}
