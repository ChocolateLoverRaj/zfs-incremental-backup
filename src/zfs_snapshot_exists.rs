use tokio::process::Command;

use crate::zfs_snapshot::ZfsSnapshot;

pub async fn zfs_snapshot_exists(zfs_snapshot: ZfsSnapshot) -> Result<bool, tokio::io::Error> {
    let output = Command::new("zfs")
        .arg("list")
        .arg("-t")
        .arg("snapshot")
        .arg(zfs_snapshot.to_string())
        .output()
        .await?;
    Ok(output.status.success())
}
