use std::{io, path::PathBuf};

use async_trait::async_trait;
use rcs3ud::{S3Dest, UploadCallbacks, UploadError2, UploadSaveData, UploadSrc2, upload_2};
use serde::{Deserialize, Serialize};
use tokio::fs::{OpenOptions, metadata, remove_file};

use crate::{
    zfs_ensure_snapshot::{ZfsEnsureSnapshotError, zfs_ensure_snapshot},
    zfs_send::{ZfsSendError, zfs_send},
    zfs_snapshot::ZfsSnapshot,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub enum BackupSaveData {
    #[default]
    CreatingSnapshot,
    SendingToFile,
    Uploading(UploadSaveData),
    RemovingFile,
}

#[async_trait]
pub trait BackupCallbacks {
    type SaveError;

    async fn save(&mut self, data: &BackupSaveData) -> Result<(), Self::SaveError>;
}

struct Callbacks<'a, SaveError> {
    callbacks: &'a mut dyn BackupCallbacks<SaveError = SaveError>,
}
impl<SaveError> UploadCallbacks for Callbacks<'_, SaveError> {
    type ReserveError = ();
    type MarkUsedError = ();
    type SaveError = SaveError;
    async fn save(&mut self, data: &UploadSaveData) -> Result<(), Self::SaveError> {
        self.callbacks
            .save(&BackupSaveData::Uploading(data.clone()))
            .await?;
        Ok(())
    }
}

#[allow(unused)]
#[derive(Debug)]
pub enum BackupError<C: BackupCallbacks> {
    Snapshot(ZfsEnsureSnapshotError),
    Save(C::SaveError),
    Open(io::Error),
    Send(ZfsSendError),
    Metadata(io::Error),
    Upload(UploadError2<(), (), C::SaveError>),
    RemoveFile(io::Error),
}

/// Takes a snapshot, does `zfs send -w` to a file, and then uploads the file to S3.
/// Can be incremental from a previous snapshot.
pub async fn backup<C: BackupCallbacks>(
    mut save_data: BackupSaveData,
    zfs_snapshot: ZfsSnapshot,
    diff_from: Option<String>,
    file_path: PathBuf,
    callbacks: &mut C,
    dest: S3Dest<'_>,
    client: &aws_sdk_s3::Client,
) -> Result<(), BackupError<C>> {
    if matches!(save_data, BackupSaveData::CreatingSnapshot) {
        zfs_ensure_snapshot(zfs_snapshot.clone())
            .await
            .map_err(BackupError::Snapshot)?;
        save_data = BackupSaveData::SendingToFile;
        callbacks
            .save(&save_data)
            .await
            .map_err(BackupError::Save)?;
    }
    if matches!(save_data, BackupSaveData::SendingToFile) {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)
            .await
            .map_err(BackupError::Open)?;
        zfs_send(zfs_snapshot, diff_from, file.into_std().await.into())
            .await
            .map_err(BackupError::Send)?;
        save_data = BackupSaveData::Uploading(Default::default());
        callbacks
            .save(&save_data)
            .await
            .map_err(BackupError::Save)?;
    }
    if let BackupSaveData::Uploading(upload_save_data) = save_data {
        let len = metadata(&file_path)
            .await
            .map_err(BackupError::Metadata)?
            .len();
        upload_2(
            client,
            UploadSrc2 {
                path: &file_path,
                offset: 0,
                len: len.try_into().unwrap(),
            },
            dest,
            &mut Callbacks { callbacks },
            Default::default(),
            upload_save_data,
        )
        .await
        .map_err(BackupError::Upload)?;
        save_data = BackupSaveData::RemovingFile;
        callbacks
            .save(&save_data)
            .await
            .map_err(BackupError::Save)?;
    }
    if let BackupSaveData::RemovingFile = save_data {
        remove_file(&file_path)
            .await
            .map_err(BackupError::RemoveFile)?;
    }
    Ok(())
}
