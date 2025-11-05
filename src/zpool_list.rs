use std::collections::HashMap;

use serde::Deserialize;
use tokio::process::Command;

use crate::command_error::CommandError;

#[derive(Debug, Deserialize)]
struct ZpoolListOutput {
    pools: HashMap<String, ()>,
}

#[allow(unused)]
#[derive(Debug)]
pub enum ZpoolListError {
    Command(CommandError),
    Deserialize(serde_json::Error),
}

pub async fn zpool_list() -> Result<Vec<String>, ZpoolListError> {
    let output = Command::new("zpool")
        .arg("list")
        .arg("--json")
        .output()
        .await
        .map_err(|e| ZpoolListError::Command(CommandError::Io(e)))?;
    if !output.status.success() {
        return Err(ZpoolListError::Command(CommandError::ExitStatus(
            output.status,
        )));
    }
    // If there are no pools, `zpool list --json` returns empty string
    if output.stdout.is_empty() {
        Ok(Default::default())
    } else {
        let output = serde_json::from_slice::<ZpoolListOutput>(&output.stdout)
            .map_err(ZpoolListError::Deserialize)?;
        Ok(output.pools.into_keys().collect())
    }
}
