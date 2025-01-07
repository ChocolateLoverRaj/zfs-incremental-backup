use std::{borrow::Cow, path::PathBuf, rc::Rc};

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use shallowclone::ShallowClone;

use crate::{
    backup_data::{BackupData, BackupStep},
    backup_steps::BackupSteps,
    get_config::get_config,
    get_data::{get_data, write_data},
    retry_steps_2::{retry_with_steps_2, StateSaver2},
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
struct BackupStateSaver<'a> {
    backup_data_path: PathBuf,
    backup_data_without_step: Rc<BackupData<'a>>,
}

impl<'a> StateSaver2<BackupStep<'a>, anyhow::Error> for BackupStateSaver<'a> {
    async fn save_state(&self, state: &BackupStep<'a>) -> Result<(), anyhow::Error> {
        Ok(write_data(
            &self.backup_data_path,
            &BackupData {
                backup_step: Some(state.shallow_clone()),
                ..self.backup_data_without_step.shallow_clone()
            },
        )
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
    let backup_data = Rc::new(get_data(&data_path).await?);
    if backup_data.backup_step.is_some() {
        Err(anyhow!(
            "Failed backup in progress. Use the continue command to continue in progress backup."
        ))?;
    }
    // Note that this only checks the last saved snapshot and there could still be backups that are already uploaded
    if backup_data.last_saved_snapshot_name.is_some()
        && backup_data.last_saved_snapshot_name.as_deref() == snapshot_name.as_deref()
    {
        Err(anyhow!("Already saved a snapshot with that name"))?;
    }
    let mut backup_steps = BackupSteps {
        config: backup_config,
        backup_data: backup_data.clone(),
    };
    let state = backup_steps
        .start(
            take_snapshot,
            snapshot_name.map(|name| Cow::Owned(name)),
            allow_empty,
        )
        .await?;
    let did_backup = retry_with_steps_2(
        state,
        &mut backup_steps,
        &mut BackupStateSaver {
            backup_data_path: data_path.clone(),
            backup_data_without_step: backup_data.clone(),
        },
    )
    .await?;
    if let Some(snapshot_name) = did_backup {
        write_data(
            &data_path,
            &BackupData {
                last_saved_snapshot_name: Some(snapshot_name),
                ..backup_data.shallow_clone()
            },
        )
        .await?;
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
    let backup_data = Rc::new(get_data(&data_path).await?);
    match &backup_data.backup_step {
        Some(backup_step) => {
            let last_saved_snapshot_name = retry_with_steps_2(
                crate::retry_steps_2::RetryStepNotFinished2 {
                    memory_data: Default::default(),
                    persistent_data: backup_step.shallow_clone(),
                },
                &mut BackupSteps {
                    config: backup_config,
                    backup_data: backup_data.clone(),
                },
                &mut BackupStateSaver {
                    backup_data_path: data_path.clone(),
                    backup_data_without_step: backup_data.clone(),
                },
            )
            .await?
            // Will never panic because will never be None
            .unwrap();
            write_data(
                &data_path,
                &BackupData {
                    backup_step: None,
                    last_saved_snapshot_name: Some(last_saved_snapshot_name),
                    ..backup_data.shallow_clone()
                },
            )
            .await?;
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
