use std::{borrow::Cow, num::NonZero, path::Path};

use aws_sdk_s3::types::StorageClass;
use rcs3ud::{AmountLimiter2, OperationScheduler2, S3Dest};
use serde::{Deserialize, Serialize};

use crate::{
    backup::{BackupError, BackupSaveData, backup},
    zfs_dataset::ZfsDataset,
    zfs_snapshot::ZfsSnapshot,
};

/// Actual data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AutoBackupState {
    pub snapshots_backed_up: usize,
    pub backing_up_progress: Option<BackupSaveData>,
}

#[derive(Debug)]
pub enum AutoBackError<ReserveError, MarkUsedError, SaveError> {
    Backup(BackupError<ReserveError, MarkUsedError, SaveError>),
    Save(SaveError),
}

/// Takes a snapshot and backs it up, or completes the previous unfinished operation.
/// The snapshot name is automatic and incremental starting at 0.
/// Always does an incremental backup from the last backed up snapshot.
pub async fn auto_back<ReserveError, MarkUsedError, SaveError>(
    mut save_data: AutoBackupState,
    dataset: ZfsDataset,
    bucket: &str,
    snapshot_prefix: &str,
    object_prefix: &str,
    temp_dir: &Path,
    storage_class: StorageClass,
    chunk_size: NonZero<usize>,
    client: &aws_sdk_s3::Client,
    amount_limiter: &mut Box<
        dyn AmountLimiter2<ReserveError = ReserveError, MarkUsedError = MarkUsedError> + Send,
    >,
    operation_scheduler: &mut Box<dyn OperationScheduler2 + Send>,
    save: &mut impl AsyncFnMut(&AutoBackupState) -> Result<(), SaveError>,
) -> Result<(), AutoBackError<ReserveError, MarkUsedError, SaveError>> {
    if save_data.backing_up_progress.is_none() {
        save_data.backing_up_progress = Some(Default::default());
    }
    let snapshot_number = save_data.snapshots_backed_up;
    let previous_snapshot_name = save_data
        .snapshots_backed_up
        .checked_sub(1)
        .map(|snapshot_number| format!("{snapshot_prefix}{snapshot_number}"));
    let snapshot_name = format!("{snapshot_prefix}{snapshot_number}");
    let object_name = if let Some(previous_snapshot_name) = &previous_snapshot_name {
        Cow::Owned(format!("{previous_snapshot_name}_{snapshot_name}"))
    } else {
        Cow::Borrowed(&snapshot_name)
    };
    let file_path = temp_dir.join(object_name.to_string());
    let object_key = format!("{object_prefix}{object_name}");
    backup(
        save_data.backing_up_progress.clone().unwrap_or_default(),
        ZfsSnapshot {
            zpool: &dataset.zpool,
            dataset: &dataset.dataset,
            snapshot_name: &snapshot_name,
        },
        previous_snapshot_name.as_deref(),
        &file_path,
        S3Dest {
            bucket: &bucket,
            object_key: &object_key,
            storage_class,
        },
        client,
        amount_limiter,
        operation_scheduler,
        chunk_size,
        &mut async |backup_save_data| {
            save_data.backing_up_progress = Some(backup_save_data.clone());
            save(&save_data).await
        },
    )
    .await
    .map_err(AutoBackError::Backup)?;
    save_data.snapshots_backed_up += 1;
    save_data.backing_up_progress = None;
    save(&save_data).await.map_err(AutoBackError::Save)?;
    Ok(())
}
