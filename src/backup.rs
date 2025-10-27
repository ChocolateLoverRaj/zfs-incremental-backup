use std::{io, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;

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
    Uploading,
}

pub trait BackupCallbacks {
    type SaveError;

    async fn save(&mut self, data: &BackupSaveData) -> Result<(), Self::SaveError>;
}

// pub struct BackupInput {
//     /// The previously saved save data. If none was saved, use `Default::default()`.
//     save_data: BackupSaveData,
//     /// The snapshot take and back up
//     snapshot: ZfsSnapshot,
//     /// The snapshot to do `-i` with `zfs send`
//     diff_from: Option<String>,
//     /// Where to temporarily store the snapshot file
//     file_path: String,
// }

#[derive(Debug)]
pub enum BackupError<C: BackupCallbacks> {
    Snapshot(ZfsEnsureSnapshotError),
    Save(C::SaveError),
    Open(io::Error),
    Send(ZfsSendError),
}

/// Takes a snapshot, does `zfs send -w` to a file, and then uploads the file to S3.
/// Can be incremental from a previous snapshot.
pub async fn backup<C: BackupCallbacks>(
    mut save_data: BackupSaveData,
    zfs_snapshot: ZfsSnapshot,
    diff_from: Option<String>,
    file_path: PathBuf,
    callbacks: &mut C,
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
            .open(file_path)
            .await
            .map_err(BackupError::Open)?;
        zfs_send(zfs_snapshot, diff_from, file.into_std().await.into())
            .await
            .map_err(BackupError::Send)?;
        save_data = BackupSaveData::Uploading;
        callbacks
            .save(&save_data)
            .await
            .map_err(BackupError::Save)?;
    }
    if matches!(save_data, BackupSaveData::Uploading) {
        println!("TODO: Upload to S3");
    }
    Ok(())
}
