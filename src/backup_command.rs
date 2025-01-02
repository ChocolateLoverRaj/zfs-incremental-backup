use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};

use crate::{
    backup_data::BackupData,
    backup_steps::BackupSteps,
    get_config::get_config,
    get_data::{get_data, write_data},
    retry_steps::{retry_with_steps, StateSaver},
};

#[derive(Parser)]
pub struct BackupStatusCommand {
    /// Path to the backup data JSON file
    #[arg(short, long)]
    data_path: PathBuf,
}

#[derive(Parser)]
pub struct BackupStartCommand {
    /// Path to a JSON file with config
    #[arg(short, long)]
    config_path: PathBuf,
    /// Path to the backup data JSON file
    #[arg(short, long)]
    data_path: PathBuf,
    /// Snapshot name (or id, if it already exists)
    #[arg(short, long)]
    snapshot_name: Option<String>,
    /// If this is `true`, a snapshot will be taken with the name
    #[arg(short, long)]
    take_snapshot: bool,
}

#[derive(Subcommand)]
pub enum BackupCommand {
    Status(BackupStatusCommand),
    Start(BackupStartCommand),
}

pub async fn backup_status_command(
    BackupStatusCommand { data_path }: BackupStatusCommand,
) -> anyhow::Result<()> {
    let backup_data = get_data(&data_path).await?;
    println!(
        "Last saved snapshot name: {:?}",
        &backup_data.last_saved_snapshot_name
    );

    if backup_data.backup_state.is_some() {
        println!("Backup in progress");
    } else {
        println!("No backup in progress");
    }

    Ok(())
}

pub async fn backup_start_command(
    BackupStartCommand {
        config_path,
        data_path,
        snapshot_name,
        take_snapshot,
    }: BackupStartCommand,
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

pub async fn backup_commands(backup_command: BackupCommand) -> anyhow::Result<()> {
    match backup_command {
        BackupCommand::Start(command) => backup_start_command(command).await?,
        BackupCommand::Status(command) => backup_status_command(command).await?,
    }
    Ok(())
}
