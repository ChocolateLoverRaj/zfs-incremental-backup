use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use aws_config::BehaviorVersion;
use aws_sdk_s3::types::BucketLocationConstraint;
use backup_data::{BackupState, EncryptionData};
use check_key::check_key;
use chrono::Utc;
use clap::{Parser, Subcommand};
use config::ENCRYPTION_DATA_OBJECT_KEY;
use diff_or_first::diff_or_first;
use get_config::get_config;
use get_data::{get_data, write_data};
use init::init;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

mod backup_config;
mod backup_data;
mod check_key;
mod config;
mod create_bucket;
mod derive_key;
mod diff_or_first;
mod encryption_password;
mod get_config;
mod get_data;
mod init;
mod read_dir_recursive;
mod serde_file;
mod zfs_mount_get;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// does testing things
    Init {
        /// The bucket name will be the prefix with a GUID. One example of a bucket name is ``. Must follow https://docs.aws.amazon.com/AmazonS3/latest/userguide/bucketnamingrules.html or you will get an error.
        #[arg(short, long, default_value = "zfs-backup")]
        bucket_prefix: String,
        #[arg(short, long, default_value = "us-west-2")]
        region: BucketLocationConstraint,
        /// Path to a JSON file with config
        #[arg(short, long)]
        config_path: PathBuf,
        /// Path to the backup data JSON file
        #[arg(short, long)]
        data_path: PathBuf,
        /// Overwrite the data file
        #[arg(short, long)]
        force: bool,
    },
    Backup {
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
    },
    CheckPassword {
        /// Path to a JSON file with config
        #[arg(short, long)]
        config_path: PathBuf,
        /// Path to the backup data JSON file
        #[arg(short, long)]
        data_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            bucket_prefix,
            region,
            config_path,
            data_path,
            force,
        } => {
            let mut backup_data_file = OpenOptions::new()
                .read(false)
                .write(true)
                .create(true)
                .truncate(true)
                .create_new(!force)
                .open(data_path)
                .await
                .map_err(|e| match e.kind() {
                    ErrorKind::AlreadyExists => anyhow::Error::from(e)
                        .context("Backup data file already exists. Use -f to overwrite."),
                    _ => e.into(),
                })?;
            let config = get_config(config_path).await?;
            let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
            let s3_client = aws_sdk_s3::Client::new(&sdk_config);
            let backup_data = init(
                &s3_client,
                &bucket_prefix,
                &region,
                &match &config.encryption_password {
                    Some(encryption_password) => Some(encryption_password.get_bytes().await?),
                    None => None,
                },
            )
            .await?;
            backup_data_file
                .write_all(serde_json::to_string_pretty(&backup_data)?.as_bytes())
                .await?;
            println!("Saved backup data: {:#?}", backup_data);
        }
        Commands::Backup {
            config_path,
            data_path,
            snapshot_name,
            take_snapshot,
        } => {
            let config = get_config(config_path).await?;
            let mut data = get_data(&data_path).await?;
            if data.backup_state.is_some() {
                Err(anyhow!("Previous backup in progress!"))?;
            };
            let snapshot_name = if take_snapshot {
                // Don't backup more than once a second please. It won't work.
                let snapshot_name = snapshot_name
                    .unwrap_or(format!("backup-{}", Utc::now().format("%Y-%m-%d_%H-%M-%S")));
                println!("Snapshot name: {snapshot_name:?}");
                let output = Command::new("zfs")
                    .arg("snapshot")
                    .arg(format!("{}@{}", config.zfs_dataset_name, snapshot_name))
                    .output()
                    .await?;
                if !output.status.success() {
                    Err(anyhow!("ZFS command failed: {output:#?}"))?;
                }
                println!("Took snapshot");
                snapshot_name
            } else {
                snapshot_name.ok_or(anyhow!(
                    "Must specify a snapshot name, or use --take-snapshot"
                ))?
            };
            println!("Diffing...");
            let diff = diff_or_first(
                config.zfs_dataset_name,
                data.last_saved_snapshot_name.as_deref(),
                snapshot_name,
            )
            .await?;
            println!("Diff: {diff:#?}");
            data.backup_state = Some(BackupState { diff });
            write_data(data_path, &data).await?;
        }
        Commands::CheckPassword {
            config_path,
            data_path,
        } => {
            let config = get_config(config_path).await?;
            let data = get_data(data_path).await?;
            match config.encryption_password {
                Some(encryption_password) => {
                    let encryption_password = encryption_password.get_bytes().await?;

                    let check_local = || {
                        check_key(
                            &encryption_password,
                            &data.encryption.ok_or(anyhow!("No salt in data"))?,
                        )?;
                        anyhow::Ok("The password worked on the local backup data")
                    };

                    let check_remote = || async {
                        let sdk_config =
                            aws_config::defaults(BehaviorVersion::latest()).load().await;
                        let s3_client = aws_sdk_s3::Client::new(&sdk_config);
                        let output = s3_client
                            .get_object()
                            .bucket(data.s3_bucket)
                            .key(ENCRYPTION_DATA_OBJECT_KEY)
                            .send()
                            .await?;
                        let s3_encryption_data = output.body.collect().await?;
                        let s3_encryption_data = postcard::from_bytes::<EncryptionData>(
                            &s3_encryption_data.into_bytes(),
                        )?;
                        check_key(&encryption_password, &s3_encryption_data)?;
                        anyhow::Ok("The password worked on the remote backup data")
                    };

                    let results = [
                        check_local().context("The password did not work on local data"),
                        check_remote()
                            .await
                            .context("The password did not work on remote data"),
                    ]
                    .into_iter()
                    .collect::<Vec<_>>();
                    println!("{:#?}", results);
                    results.into_iter().collect::<Result<Vec<_>, _>>()?;
                }
                None => {
                    Err(anyhow!("Not encrypted"))?;
                }
            }
        }
    }
    Ok(())
}
