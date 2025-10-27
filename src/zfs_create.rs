use tokio::process::Command;

use crate::{command_error::CommandError, zfs_dataset::ZfsDataset};

pub async fn zfs_create(zfs_dataset: ZfsDataset) -> Result<(), CommandError> {
    let exit_status = Command::new("zfs")
        .arg("create")
        .arg(zfs_dataset.to_string())
        .output()
        .await
        .map_err(CommandError::Io)?
        .status;
    if !exit_status.success() {
        return Err(CommandError::ExitStatus(exit_status));
    }
    Ok(())
}
