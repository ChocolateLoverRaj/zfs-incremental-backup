use std::{io::ErrorKind, path::PathBuf};

use aws_config::BehaviorVersion;
use aws_sdk_s3::types::BucketLocationConstraint;
use clap::Parser;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

use crate::{get_config::get_config, init::init};

#[derive(Parser)]
pub struct InitCommand {
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
}

pub async fn init_command(
    InitCommand {
        bucket_prefix,
        region,
        config_path,
        data_path,
        force,
    }: InitCommand,
) -> anyhow::Result<()> {
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
        &match &config.encryption {
            Some(encryption_config) => Some(encryption_config.password.get_bytes().await?),
            None => None,
        },
    )
    .await?;
    backup_data_file
        .write_all(serde_json::to_string_pretty(&backup_data)?.as_bytes())
        .await?;
    println!("Saved backup data: {:#?}", backup_data);
    Ok(())
}
