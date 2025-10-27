use std::process::ExitStatus;

use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ZfsSnapshot {
    pub zpool: String,
    pub dataset: String,
    pub snapshot_name: String,
}

#[derive(Debug)]
pub enum ZfsTakeSnapshotError {
    CommandError(tokio::io::Error),
    ErrStatus(ExitStatus),
}

pub async fn zfs_take_snapshot(
    ZfsSnapshot {
        zpool,
        dataset,
        snapshot_name,
    }: ZfsSnapshot,
) -> Result<(), ZfsTakeSnapshotError> {
    let output = Command::new("zfs")
        .arg("snapshot")
        .arg(format!("{zpool}/{dataset}@{snapshot_name}"))
        .output()
        .await
        .map_err(ZfsTakeSnapshotError::CommandError)?;
    if !output.status.success() {
        return Err(ZfsTakeSnapshotError::ErrStatus(output.status));
    }
    Ok(())
}
