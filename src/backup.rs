use std::{io, num::NonZeroUsize, path::Path};

use rcs3ud::{
    AmountLimiter2, OperationScheduler2, S3Dest, UploadChunkedError2, UploadChunkedSaveData2,
    upload_chunked_2,
};
use serde::{Deserialize, Serialize};
use tokio::fs::{OpenOptions, remove_file};

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
    Uploading(UploadChunkedSaveData2),
    RemovingFile,
}

#[allow(unused)]
#[derive(Debug)]
pub enum BackupError<ReserveError, MarkUsedError, SaveError> {
    Snapshot(ZfsEnsureSnapshotError),
    Save(SaveError),
    Open(io::Error),
    Send(ZfsSendError),
    Upload(UploadChunkedError2<ReserveError, MarkUsedError, SaveError>),
    RemoveFile(io::Error),
}

/// Takes a snapshot, does `zfs send -w` to a file, and then uploads the file to S3.
/// Can be incremental from a previous snapshot.
pub async fn backup<ReserveError, MarkUsedError, SaveError>(
    mut save_data: BackupSaveData,
    zfs_snapshot: ZfsSnapshot,
    diff_from: Option<String>,
    file_path: &Path,
    dest: S3Dest<'_>,
    client: &aws_sdk_s3::Client,
    amount_limiter: &mut Box<
        dyn AmountLimiter2<ReserveError = ReserveError, MarkUsedError = MarkUsedError> + Send,
    >,
    operation_scheduler: &mut Box<dyn OperationScheduler2 + Send>,
    chunk_size: NonZeroUsize,
    save: &mut impl AsyncFnMut(&BackupSaveData) -> Result<(), SaveError>,
) -> Result<(), BackupError<ReserveError, MarkUsedError, SaveError>> {
    if matches!(save_data, BackupSaveData::CreatingSnapshot) {
        zfs_ensure_snapshot(zfs_snapshot.clone())
            .await
            .map_err(BackupError::Snapshot)?;
        save_data = BackupSaveData::SendingToFile;
        save(&save_data).await.map_err(BackupError::Save)?;
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
        save(&save_data).await.map_err(BackupError::Save)?;
    }
    if let BackupSaveData::Uploading(upload_save_data) = &save_data {
        upload_chunked_2(
            client,
            file_path,
            dest,
            chunk_size,
            upload_save_data.clone(),
            amount_limiter,
            operation_scheduler,
            &mut async |upload_save_data| {
                save_data = BackupSaveData::Uploading(upload_save_data.clone());
                save(&save_data).await?;
                Ok(())
            },
        )
        .await
        .map_err(BackupError::Upload)?;
        save_data = BackupSaveData::RemovingFile;
        save(&save_data).await.map_err(BackupError::Save)?;
    }
    if let BackupSaveData::RemovingFile = save_data {
        remove_file(&file_path)
            .await
            .map_err(BackupError::RemoveFile)?;
    }
    Ok(())
}
