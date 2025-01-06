use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};

use crate::{
    backup_data::{BackupData, BackupStep},
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
    /// Even if no files were changed, still backup the snapshot.
    /// This is useful if you want to backup a new snapshot name for some reason.
    #[arg(short, long)]
    allow_empty: bool,
}

#[derive(Parser)]
pub struct BackupContinueCommand {
    /// Path to a JSON file with config
    #[arg(short, long)]
    config_path: PathBuf,
    /// Path to the backup data JSON file
    #[arg(short, long)]
    data_path: PathBuf,
}

#[derive(Subcommand)]
pub enum BackupCommand {
    Status(BackupStatusCommand),
    Start(BackupStartCommand),
    Continue(BackupContinueCommand),
}

pub async fn backup_status_command(
    BackupStatusCommand { data_path }: BackupStatusCommand,
) -> anyhow::Result<()> {
    let backup_data = get_data(&data_path).await?;
    println!(
        "Last saved snapshot name: {:?}",
        &backup_data.last_saved_snapshot_name
    );

    if backup_data.backup_step.is_some() {
        println!("Backup in progress");
    } else {
        println!("No backup in progress");
    }

    Ok(())
}

// TODO: impl the trait for a closure so we don't have to make this struct and implement it for the struct
struct BackupStateSaver {
    backup_data_path: PathBuf,
    backup_data_without_step: BackupData,
}

impl StateSaver<BackupStep, anyhow::Error> for BackupStateSaver {
    async fn save_state<'a>(&'a mut self, state: &'a BackupStep) -> Result<(), anyhow::Error> {
        Ok(write_data(&self.backup_data_path, &{
            let mut backup_data = self.backup_data_without_step.clone();
            backup_data.backup_step = Some(state.clone());
            backup_data
        })
        .await?)
    }
}

pub async fn backup_start_command(
    BackupStartCommand {
        config_path,
        data_path,
        snapshot_name,
        take_snapshot,
        allow_empty,
    }: BackupStartCommand,
) -> anyhow::Result<()> {
    let backup_config = get_config(&config_path).await?;
    let backup_data = get_data(&data_path).await?;
    if backup_data.backup_step.is_some() {
        Err(anyhow!("Failed backup in progress. It can be continued / retried, but the command to continue failed backup not implemented yet."))?;
    }
    let backup_steps = BackupSteps {
        config: backup_config,
        last_saved_snapshot_name: backup_data.last_saved_snapshot_name.clone(),
        s3_bucket: backup_data.s3_bucket.clone(),
    };
    let backup_data_without_step = backup_data;
    let did_backup = retry_with_steps(
        backup_steps
            .start(take_snapshot, snapshot_name, allow_empty)
            .await?,
        backup_steps,
        BackupStateSaver {
            backup_data_path: data_path.clone(),
            backup_data_without_step: backup_data_without_step.clone(),
        },
    )
    .await?;
    if did_backup {
        write_data(&data_path, &backup_data_without_step).await?;
    } else {
        println!("Did not take backup because there was nothing new to back up");
    }
    Ok(())
}

pub async fn backup_continue_command(
    BackupContinueCommand {
        config_path,
        data_path,
    }: BackupContinueCommand,
) -> anyhow::Result<()> {
    let backup_config = get_config(&config_path).await?;
    let backup_data = get_data(&data_path).await?;
    match backup_data.backup_step {
        Some(backup_step) => {
            let backup_data_without_step = BackupData {
                backup_step: None,
                ..backup_data
            };
            retry_with_steps(
                backup_step,
                BackupSteps {
                    config: backup_config,
                    last_saved_snapshot_name: backup_data_without_step
                        .last_saved_snapshot_name
                        .clone(),
                    s3_bucket: backup_data_without_step.s3_bucket.clone(),
                },
                BackupStateSaver {
                    backup_data_path: data_path.clone(),
                    backup_data_without_step: backup_data_without_step.clone(),
                },
            )
            .await?;
            write_data(&data_path, &backup_data_without_step).await?;
        }
        None => {
            Err(anyhow!("No backup in progress"))?;
        }
    }
    Ok(())
}

pub async fn backup_commands(backup_command: BackupCommand) -> anyhow::Result<()> {
    match backup_command {
        BackupCommand::Start(command) => backup_start_command(command).await?,
        BackupCommand::Status(command) => backup_status_command(command).await?,
        BackupCommand::Continue(command) => backup_continue_command(command).await?,
    }
    Ok(())
}
