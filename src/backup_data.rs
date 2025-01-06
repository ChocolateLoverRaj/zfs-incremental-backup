use serde::{Deserialize, Serialize};

use crate::{diff_or_first::DiffEntry, file_meta_data::FileMetaData};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupStepDiff {
    pub snapshot_name: String,
    pub allow_empty: bool,
}

impl BackupStepDiff {
    pub fn next(self, diff: Vec<DiffEntry<Option<FileMetaData>>>) -> BackupStep {
        BackupStep::Upload(BackupStepUpload {
            snapshot_name: self.snapshot_name,
            diff,
            uploaded_objects: 0,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupStepUpload {
    pub snapshot_name: String,
    pub diff: Vec<DiffEntry<Option<FileMetaData>>>,
    pub uploaded_objects: u64,
}

impl BackupStepUpload {
    pub fn next(self) -> BackupStep {
        BackupStep::UpdateHotData(BackupStepUpdateHotData {
            snapshot_name: self.snapshot_name,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupStepUpdateHotData {
    pub snapshot_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BackupStep {
    Diff(BackupStepDiff),
    Upload(BackupStepUpload),
    UpdateHotData(BackupStepUpdateHotData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupState {
    pub snapshot_name: String,
    pub stage: BackupStep,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupData {
    pub s3_bucket: String,
    /// Idk if we will use this but it would be useful in case the region chances in the local AWS credentials / config file
    pub s3_region: String,
    pub last_saved_snapshot_name: Option<String>,
    pub backup_step: Option<BackupStep>,
}
