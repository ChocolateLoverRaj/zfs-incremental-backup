use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use shallowclone::ShallowClone;

use crate::{diff_or_first::DiffEntry, file_meta_data::FileMetaData};

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub struct BackupStepDiff<'a> {
    pub snapshot_name: Cow<'a, str>,
    pub allow_empty: bool,
    // pub hot_data: RemoteHotDataDecrypted<'a>,
}

impl<'a> BackupStepDiff<'a> {
    pub fn next(self, diff: Vec<DiffEntry<Option<FileMetaData>>>) -> BackupStep<'a> {
        BackupStep::Upload(BackupStepUpload {
            snapshot_name: self.snapshot_name,
            diff: Cow::Owned(diff),
            uploaded_objects: 0,
            // hot_data: self.hot_data,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub struct BackupStepUpload<'a> {
    pub snapshot_name: Cow<'a, str>,
    pub diff: Cow<'a, Vec<DiffEntry<Option<FileMetaData>>>>,
    pub uploaded_objects: u64,
    // pub hot_data: RemoteHotData<'a>,
}

impl<'a> BackupStepUpload<'a> {
    pub fn next(self) -> BackupStep<'a> {
        BackupStep::UpdateHotData(BackupStepUpdateHotData {
            snapshot_name: self.snapshot_name,
            // hot_data: self.hot_data,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub struct BackupStepUpdateHotData<'a> {
    pub snapshot_name: Cow<'a, str>,
    // pub hot_data: RemoteHotData<'a>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub enum BackupStep<'a> {
    Diff(BackupStepDiff<'a>),
    Upload(BackupStepUpload<'a>),
    UpdateHotData(BackupStepUpdateHotData<'a>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupState<'a> {
    pub snapshot_name: String,
    pub stage: BackupStep<'a>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub struct BackupData<'a> {
    pub s3_bucket: Cow<'a, str>,
    /// Idk if we will use this but it would be useful in case the region chances in the local AWS credentials / config file
    pub s3_region: Cow<'a, str>,
    pub last_saved_snapshot_name: Option<Cow<'a, str>>,
    pub backup_step: Option<BackupStep<'a>>,
}
