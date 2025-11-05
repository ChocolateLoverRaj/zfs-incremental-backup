use std::process::ExitStatus;

use tokio::process::Command;

use crate::zfs_snapshot::ZfsSnapshot;

#[allow(unused)]
#[derive(Debug)]
pub enum ZfsTakeSnapshotError {
    CommandError(tokio::io::Error),
    ErrStatus(ExitStatus),
}

pub async fn zfs_take_snapshot(zfs_snapshot: ZfsSnapshot) -> Result<(), ZfsTakeSnapshotError> {
    let output = Command::new("zfs")
        .arg("snapshot")
        .arg(zfs_snapshot.to_string())
        .output()
        .await
        .map_err(ZfsTakeSnapshotError::CommandError)?;
    if !output.status.success() {
        return Err(ZfsTakeSnapshotError::ErrStatus(output.status));
    }
    Ok(())
}
