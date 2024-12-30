use std::path::Path;

use tokio::fs::read_to_string;

use crate::backup_config::BackupConfig;

pub async fn get_config(config_path: impl AsRef<Path>) -> anyhow::Result<BackupConfig> {
    let config = read_to_string(config_path).await?;
    let config = serde_json::from_str::<BackupConfig>(&config)?;
    Ok(config)
}
