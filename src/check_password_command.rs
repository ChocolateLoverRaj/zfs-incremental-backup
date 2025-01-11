use std::path::PathBuf;

use anyhow::{anyhow, Context};
use aws_config::BehaviorVersion;
use clap::Parser;

use crate::{
    decrypt_immutable_key::decrypt_immutable_key, get_config::get_config, get_data::get_data,
    remote_hot_data::download_hot_data,
};

#[derive(Parser)]
pub struct CheckPasswordCommand {
    /// Path to a JSON file with config
    #[arg(short, long)]
    config_path: PathBuf,
    /// Path to the backup data JSON file
    #[arg(short, long)]
    data_path: PathBuf,
}

pub async fn check_password_command(
    CheckPasswordCommand {
        config_path,
        data_path,
    }: CheckPasswordCommand,
) -> anyhow::Result<()> {
    let config = get_config(config_path).await?;
    let backup_data = get_data(data_path).await?;
    let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);
    let remote_hot_data = download_hot_data(&config, &s3_client, &backup_data.s3_bucket).await?;
    match remote_hot_data.encryption {
        Some(encryption) => match config.encryption {
            Some(encryption_config) => {
                let encryption_password = encryption_config.password.get_bytes().await?;

                decrypt_immutable_key(&encryption_password, &encryption)
                    .context("The password did not work on the remote backup data")?;
                println!("The password worked on the remote backup data");
            }
            None => {
                Err(anyhow!("The remote data is encrypted, but the local config does not include a password. In this current state, you will not be able to recover the data."))?;
            }
        },
        None => match config.encryption {
            None => {
                println!("No password set. Not encrypted. No password needed to restore.");
            }
            Some(_) => {
                println!("A password is set in the config, but the remote data is not encrypted. This indicates a mismatch between the config and the remote data.");
            }
        },
    }
    Ok(())
}
