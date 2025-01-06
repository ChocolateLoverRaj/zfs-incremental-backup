use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use aws_config::BehaviorVersion;
use aws_sdk_s3::types::BucketLocationConstraint;
use backup_command::{backup_commands, BackupCommand};
use check_key::decrypt_immutable_key;
use clap::{Parser, Subcommand};
use derive_key::{encrypt_immutable_key, generate_salt_and_derive_key};
use get_config::get_config;
use get_data::get_data;
use init::init;
use promptuity::prompts::Password;
use promptuity::themes::MinimalTheme;
use promptuity::{Promptuity, Term};
use remote_hot_data::{download_hot_data, upload_hot_data, EncryptionData};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

mod aws_s3_prices;
mod backup_command;
mod backup_config;
mod backup_data;
mod backup_steps;
mod check_key;
mod chunks_stream;
mod config;
mod create_bucket;
mod derive_key;
mod diff_or_first;
mod encryption_password;
mod file_meta_data;
mod get_config;
mod get_data;
mod init;
mod read_dir_recursive;
mod remote_hot_data;
mod retry_steps;
mod serde_file;
mod snapshot_upload_stream_2;
mod zfs_mount_get;
mod zfs_take_snapshot;

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
        #[command(subcommand)]
        command: BackupCommand,
    },
    CheckPassword {
        /// Path to a JSON file with config
        #[arg(short, long)]
        config_path: PathBuf,
        /// Path to the backup data JSON file
        #[arg(short, long)]
        data_path: PathBuf,
    },
    ChangePassword {
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
        Commands::Backup { command } => backup_commands(command).await?,
        Commands::CheckPassword {
            config_path,
            data_path,
        } => {
            let config = get_config(config_path).await?;
            let backup_data = get_data(data_path).await?;
            let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
            let s3_client = aws_sdk_s3::Client::new(&sdk_config);
            let remote_hot_data = download_hot_data(&s3_client, &backup_data.s3_bucket).await?;
            match remote_hot_data.encryption {
                Some(encryption) => match config.encryption_password {
                    Some(encryption_password) => {
                        let encryption_password = encryption_password.get_bytes().await?;

                        decrypt_immutable_key(&encryption_password, &encryption)
                            .context("The password did not work on the remote backup data")?;
                        println!("The password worked on the remote backup data");
                    }
                    None => {
                        Err(anyhow!("The remote data is encrypted, but the local config does not include a password. In this current state, you will not be able to recover the data."))?;
                    }
                },
                None => match config.encryption_password {
                    None => {
                        println!("No password set. Not encrypted. No password needed to restore.");
                    }
                    Some(_) => {
                        println!("A password is set in the config, but the remote data is not encrypted. This indicates a mismatch between the config and the remote data.");
                    }
                },
            }
        }
        Commands::ChangePassword {
            config_path,
            data_path,
        } => {
            let backup_config = get_config(config_path).await?;
            let backup_data = get_data(&data_path).await?;
            match backup_config.encryption_password {
                Some(encryption_password) => {
                    let encryption_password = encryption_password.get_bytes().await?;
                    let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
                    let s3_client = aws_sdk_s3::Client::new(&sdk_config);
                    let mut remote_hot_data =
                        download_hot_data(&s3_client, &backup_data.s3_bucket).await?;
                    let decrypted_immutable_key =
                        decrypt_immutable_key(&encryption_password, &remote_hot_data.encryption.ok_or(anyhow!("The local config specifies an encryption password, but the remote data is not encrypted."))?)?;

                    let mut term = Term::default();
                    let mut theme = MinimalTheme::default();
                    let mut p = Promptuity::new(&mut term, &mut theme);

                    let new_password = {
                        p.begin()?;
                        let password = loop {
                            let password =
                            p.prompt(Password::new("Set a new encryption password").with_hint(
                                "If you need an alternate method other than stdin, open an issue",
                            ))?;
                            let password_repeated = p.prompt(
                                Password::new("Re-enter the new encryption password").as_mut(),
                            )?;
                            if password_repeated == password {
                                break password;
                            }
                            p.error("Repeated password was not the same")?;
                        };
                        p.finish()?;
                        password
                    };
                    let (new_salt, new_derived_key) =
                        generate_salt_and_derive_key(new_password.as_bytes())?;
                    let encrypted_immutable_key =
                        encrypt_immutable_key(&new_derived_key, &decrypted_immutable_key)?;
                    remote_hot_data.encryption = Some(EncryptionData {
                        password_derived_key_salt: new_salt,
                        encrypted_immutable_key,
                    });
                    upload_hot_data(&s3_client, &backup_data.s3_bucket, &remote_hot_data).await?;
                    println!("Changed encryption password. Make sure to update your config to use the new password because the previous password will not work. You can use `check-password` to check it.");
                }
                None => {
                    Err(anyhow!("Not encrypted! There is no way to encrypt the unencrypted backups. You will have to create a new encrypted backup and delete the old one."))?;
                }
            }
        }
    }
    Ok(())
}
