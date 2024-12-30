use anyhow::anyhow;
use tokio::process::Command;

pub async fn zfs_take_snapshot(dataset: &str, snapshot: &str) -> anyhow::Result<()> {
    let output = Command::new("zfs")
        .arg("snapshot")
        .arg(format!("{}@{}", dataset, snapshot))
        .output()
        .await?;
    if !output.status.success() {
        Err(anyhow!("ZFS command failed: {output:#?}"))?;
    }
    Ok(())
}
