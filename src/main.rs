mod backup;
mod command_error;
mod zfs_create;
mod zfs_dataset;
mod zfs_ensure_snapshot;
mod zfs_send;
mod zfs_snapshot;
mod zfs_snapshot_exists;
mod zfs_take_snapshot;
mod zpool_create;
mod zpool_destroy;
mod zpool_ensure_destroy;
mod zpool_list;

use std::{
    io::{self, ErrorKind},
    num::NonZero,
    path::PathBuf,
};

use aws_config::Region;
use aws_sdk_s3::{config::Credentials, types::StorageClass};
use clap::Parser;
use rcs3ud::{
    AmountLimiter2, NoOpAmountLimiter2, NoOpOperationScheduler2, OperationScheduler2, S3Dest,
};
use tokio::fs::{read_to_string, remove_file, write};

use crate::{backup::backup, zfs_snapshot::ZfsSnapshot};

#[derive(Debug, Parser)]
struct Cli {
    #[arg(long)]
    zpool: String,
    #[arg(long)]
    dataset: String,
    #[arg(long)]
    snapshot: String,
    #[arg(long)]
    diff_from: Option<String>,
    #[arg(long)]
    chunk_size: NonZero<usize>,
    #[arg(long)]
    temp_path: String,
    #[arg(long)]
    save_data_path: String,
    #[arg(long)]
    bucket: String,
    #[arg(long)]
    object_key: String,
    #[arg(long, value_parser = parse_storage_class)]
    storage_class: StorageClass,
}

fn parse_storage_class(storage_class: &str) -> Result<StorageClass, String> {
    StorageClass::try_parse(storage_class).map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    let Cli {
        zpool,
        dataset,
        snapshot,
        diff_from,
        chunk_size,
        temp_path,
        save_data_path,
        bucket,
        object_key,
        storage_class,
    } = Cli::parse();
    let config = aws_sdk_s3::config::Builder::default()
        .behavior_version_latest()
        .endpoint_url("http://localhost:9000")
        .credentials_provider(Credentials::new(
            "minioadmin",
            "minioadmin",
            None,
            None,
            "minio",
        ))
        .region(Region::from_static("us-west-2"))
        .force_path_style(true)
        .build();
    let client = aws_sdk_s3::Client::from_conf(config);

    #[allow(unused)]
    #[derive(Debug)]
    enum SaveError {
        Serialize(ron::Error),
        Write(io::Error),
    }
    backup(
        match read_to_string(&save_data_path).await {
            Ok(s) => ron::from_str(&s)
                .inspect_err(|e| {
                    println!("Error parsing save data: {e}. File might have gotten corrupted")
                })
                .unwrap_or_default(),
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    Default::default()
                } else {
                    Err(e).unwrap()
                }
            }
        },
        ZfsSnapshot {
            zpool: &zpool,
            dataset: &dataset,
            snapshot_name: &snapshot,
        },
        diff_from.as_deref(),
        &PathBuf::from(temp_path),
        S3Dest {
            bucket: &bucket,
            object_key: &object_key,
            storage_class,
        },
        &client,
        &mut (Box::new(NoOpAmountLimiter2)
            as Box<dyn AmountLimiter2<ReserveError = (), MarkUsedError = ()> + Send>),
        &mut (Box::new(NoOpOperationScheduler2) as Box<dyn OperationScheduler2 + Send>),
        chunk_size,
        &mut async |save_data| {
            write(
                &save_data_path,
                ron::to_string(save_data).map_err(SaveError::Serialize)?,
            )
            .await
            .map_err(SaveError::Write)?;
            Ok::<_, SaveError>(())
        },
    )
    .await
    .unwrap();
    println!("Done");
    remove_file(&save_data_path).await.unwrap();
}
