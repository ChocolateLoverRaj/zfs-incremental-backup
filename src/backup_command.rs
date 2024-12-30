use std::path::PathBuf;

use anyhow::anyhow;

use crate::{
    backup_data::BackupData,
    backup_steps::BackupSteps,
    get_config::get_config,
    get_data::{get_data, write_data},
    retry_steps::{retry_with_steps, StateSaver},
};

pub async fn backup_command(
    config_path: PathBuf,
    data_path: PathBuf,
    snapshot_name: Option<String>,
    take_snapshot: bool,
) -> anyhow::Result<()> {
    let backup_config = get_config(&config_path).await?;
    let backup_data = get_data(&data_path).await?;
    if backup_data.backup_state.is_some() {
        Err(anyhow!("Failed backup in progress. It can be continued / retried, but the command to continue failed backup not implemented yet."))?;
    }
    retry_with_steps(
        backup_data,
        BackupSteps {
            config: backup_config,
            take_snapshot,
            snapshot_name,
        },
        {
            // TODO: impl the trait for a closure so we don't have to make this struct and implement it for the struct
            struct BackupStateSaver {
                backup_data_path: PathBuf,
            }

            impl StateSaver<BackupData, anyhow::Error> for BackupStateSaver {
                async fn save_state<'a>(
                    &'a mut self,
                    state: &'a BackupData,
                ) -> Result<(), anyhow::Error> {
                    Ok(write_data(&self.backup_data_path, state).await?)
                }
            }

            BackupStateSaver {
                backup_data_path: data_path,
            }
        },
    )
    .await?;
    Ok(())
}
