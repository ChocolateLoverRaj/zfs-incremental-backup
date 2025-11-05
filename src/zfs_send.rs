use std::process::{ExitStatus, Stdio};

use tokio::process::Command;

use crate::zfs_snapshot::ZfsSnapshot;

#[allow(unused)]
#[derive(Debug)]
pub enum ZfsSendError {
    Spawn(tokio::io::Error),
    Wait(tokio::io::Error),
    ErrorStatus(ExitStatus),
}

/// Does `zfs send -w <snapshot>`. If `diff_from` is specified, does an incremental snapshot with `-i`.
pub async fn zfs_send(
    zfs_snapshot: ZfsSnapshot<'_>,
    diff_from: Option<&str>,
    stdout: Stdio,
) -> Result<(), ZfsSendError> {
    let mut command = Command::new("zfs");
    command.arg("send").arg("-w");
    if let Some(diff_from) = diff_from {
        command.arg("-i").arg(format!("@{diff_from}"));
    }
    let exit_status = command
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
