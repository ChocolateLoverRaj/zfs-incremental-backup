use std::io::BufRead;

use anyhow::anyhow;
use tokio::process::Command;

/// `data_set`` should be in the format zpool/data_set
/// Returns names only, no other data
pub async fn zfs_list_snapshots(data_set: &str) -> anyhow::Result<Vec<String>> {
    Ok({
        let output = Command::new("zfs")
            .arg("list")
            .arg("-t")
            .arg("snapshot")
            .arg(data_set)
            .arg("-H")
            .arg("-o")
            .arg("name")
            .output()
            .await?;
        if !output.status.success() {
            Err(anyhow!("Bad status"))?;
        }
        output.stdout.lines().collect::<Result<_, _>>()?
    })
}
