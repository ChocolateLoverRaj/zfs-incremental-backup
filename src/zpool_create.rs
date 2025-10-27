use tokio::process::Command;

use crate::command_error::CommandError;

/// Create a file-backed zpool
pub async fn zpool_create(name: &str, source: &str) -> Result<(), CommandError> {
    let exit_status = Command::new("zpool")
        .arg("create")
        .arg(name)
        .arg(source)
        .output()
        .await
        .map_err(CommandError::Io)?
        .status;
    if !exit_status.success() {
        return Err(CommandError::ExitStatus(exit_status));
    }
    Ok(())
}
