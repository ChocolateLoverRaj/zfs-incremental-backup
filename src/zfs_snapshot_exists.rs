use tokio::process::Command;

use crate::zfs_take_snapshot::ZfsSnapshot;

pub async fn zfs_snapshot_exists(
    ZfsSnapshot {
        zpool,
        dataset,
        snapshot_name,
    }: ZfsSnapshot,
) -> Result<bool, tokio::io::Error> {
    let output = Command::new("zfs")
        .arg("list")
        .arg("-t")
        .arg("snapshot")
        .arg(format!("{zpool}/{dataset}@{snapshot_name}"))
        .output()
        .await?;
    Ok(output.status.success())
}
