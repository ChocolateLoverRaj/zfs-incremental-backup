use std::process::{ExitStatus, Stdio};

use tokio::process::Command;

use crate::zfs_snapshot::ZfsSnapshot;

#[derive(Debug)]
pub enum ZfsSendError {
    Spawn(tokio::io::Error),
    Wait(tokio::io::Error),
    ErrorStatus(ExitStatus),
}

/// Does `zfs send -w <snapshot>`
pub async fn zfs_send(zfs_snapshot: ZfsSnapshot, stdout: Stdio) -> Result<(), ZfsSendError> {
    let exit_status = Command::new("zfs")
        .arg("send")
        .arg("-w")
        .arg(zfs_snapshot.to_string())
        .stdout(stdout)
        .spawn()
        .map_err(ZfsSendError::Spawn)?
        .wait()
        .await
        .map_err(ZfsSendError::Wait)?;
    if !exit_status.success() {
        return Err(ZfsSendError::ErrorStatus(exit_status));
    }
    Ok(())
}
