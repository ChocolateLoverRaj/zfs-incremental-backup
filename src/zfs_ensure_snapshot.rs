use crate::{
    zfs_snapshot_exists::zfs_snapshot_exists,
    zfs_take_snapshot::{ZfsSnapshot, ZfsTakeSnapshotError, zfs_take_snapshot},
};

#[derive(Debug)]
pub enum ZfsEnsureSnapshotError {
    TakeSnapshot(ZfsTakeSnapshotError),
    SnapshotExists(tokio::io::Error),
}

#[derive(Debug)]
pub enum ZfsEnsureSnapshotOutput {
    TookSnapshot,
    SnapshotAlreadyExists,
}

/// The assumption is that no external program is interacting with the same snapshot name while this function is running.
pub async fn zfs_ensure_snapshot(
    zfs_snapshot: ZfsSnapshot,
) -> Result<ZfsEnsureSnapshotOutput, ZfsEnsureSnapshotError> {
    match zfs_take_snapshot(zfs_snapshot.clone()).await {
        Ok(()) => Ok(ZfsEnsureSnapshotOutput::TookSnapshot),
        Err(e) => {
            if zfs_snapshot_exists(zfs_snapshot)
                .await
                .map_err(ZfsEnsureSnapshotError::SnapshotExists)?
            {
                Ok(ZfsEnsureSnapshotOutput::SnapshotAlreadyExists)
            } else {
                Err(ZfsEnsureSnapshotError::TakeSnapshot(e))
            }
        }
    }
}
