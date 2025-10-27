use tokio::process::Command;

use crate::command_error::CommandError;

pub async fn zpool_destroy(zpool: &str) -> Result<(), CommandError> {
    let exit_status = Command::new("zpool")
        .arg("destroy")
        .arg(zpool)
        .output()
        .await
        .map_err(CommandError::Io)?
        .status;
    if !exit_status.success() {
        return Err(CommandError::ExitStatus(exit_status));
    }
    Ok(())
}
