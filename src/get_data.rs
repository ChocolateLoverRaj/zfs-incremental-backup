use std::path::Path;

use tokio::fs::{read_to_string, write};

use crate::backup_data::BackupData;

pub async fn get_data(data_path: impl AsRef<Path>) -> anyhow::Result<BackupData> {
    let config = read_to_string(data_path).await?;
    let config = serde_json::from_str(&config)?;
    Ok(config)
}

pub async fn write_data(data_path: impl AsRef<Path>, data: &BackupData) -> anyhow::Result<()> {
    write(data_path, serde_json::to_string_pretty(data)?).await?;
    Ok(())
}
