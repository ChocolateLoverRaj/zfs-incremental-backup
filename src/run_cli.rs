use crate::{
    init_cli::{AutoBackupConfig, AutoBackupFileData},
    parse_storage_class::parse_storage_class,
    run::run,
};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{config::Credentials, types::StorageClass};
use clap::Parser;
use rcs3ud::{AmountLimiter2, NoOpAmountLimiter2, NoOpOperationScheduler2, OperationScheduler2};
use std::{num::NonZero, path::PathBuf};
use tokio::fs::{read_to_string, write};

/// Example (what I do):
/// zpool: "para-z"
/// dataset: "immich"
/// snapshot_prefix: "backup"
/// chunk_size: 5000000000 (5 GB, the maximum object size on AWS when using PutObject and not multi-part uploads)
/// temp_dir: "/mnt/para-z/backups/immich/temp" (also in the "para-z" zpool, in a different dataset, has plenty of space)
/// save_data_path: "/mnt/para-z/backups/immich/save_data.ron"
/// bucket: "zfs-sends"
/// object_prefix: "immich/"
/// storage_class: "DEEP_ARCHIVE"
///
/// The first backup will result in a snapshot "para-z/immich@backup0" to be created, and it will be uploaded to s3://zfs-sends/immich/backup0/{0,1,2,..}.
/// The second backup will result in a snapshot "para-z/immich@backup1" to be created, and it will be uploaded to s3://zfs-sends/immich/backup0_backup1/{0,1,2,...}.
#[derive(Debug, Parser)]
pub struct Cli {
    /// A path where a single file will be saved that keeps track of the state of this program, including the last uploaded snapshot and backup progress.
    #[arg(long)]
    save_data_path: String,
    /// A place where this program can store temporary files.
    /// Entire `zfs send` outputs are saved here so most likely this will need to be backed by a HDD or SDD and not RAM.
    #[arg(long)]
    temp_dir: String,
    #[arg(long, value_parser = parse_storage_class)]
    storage_class: StorageClass,
    /// The maximum object size, in bytes. If the file is bigger than the max object size, then a file will be split up into multiple S3 objects labeled `0`, `1`, `2`, ...
    #[arg(long)]
    chunk_size: NonZero<usize>,
    /// Use development S3 server (minio)
    #[arg(long)]
    dev: bool,
    #[arg(long, default_value = "http://localhost:9000")]
    dev_endpoint: String,
}

pub async fn run_cli(
    Cli {
        save_data_path,
        temp_dir,
        storage_class,
        chunk_size,
        dev,
        dev_endpoint,
    }: Cli,
) {
    let client = if dev {
        aws_sdk_s3::Client::from_conf(
            aws_sdk_s3::config::Builder::default()
                .behavior_version_latest()
                .endpoint_url(dev_endpoint)
                .credentials_provider(Credentials::new(
                    "minioadmin",
                    "minioadmin",
                    None,
                    None,
                    "minio",
                ))
                .region(Region::from_static("us-east-1"))
                .force_path_style(true)
                .build(),
        )
    } else {
        aws_sdk_s3::Client::new(&aws_config::load_defaults(BehaviorVersion::latest()).await)
    };

    let mut file_data =
        ron::from_str::<AutoBackupFileData>(&read_to_string(&save_data_path).await.unwrap())
            .unwrap();
    let AutoBackupConfig {
        dataset,
        bucket,
        snapshot_prefix,
        object_prefix,
    } = file_data.config.clone();
    run(
        file_data.state.clone(),
        dataset,
        &bucket,
        &snapshot_prefix,
        &object_prefix,
        &PathBuf::from(temp_dir),
        storage_class,
        chunk_size,
        &client,
        &mut (Box::new(NoOpAmountLimiter2)
            as Box<dyn AmountLimiter2<ReserveError = (), MarkUsedError = ()> + Send>),
        &mut (Box::new(NoOpOperationScheduler2) as Box<dyn OperationScheduler2 + Send>),
        &mut async |state| {
            file_data.state = state.clone();
            write(
                &save_data_path,
                ron::ser::to_string_pretty(&file_data, Default::default()).unwrap(),
            )
            .await
        },
    )
    .await
    .unwrap();
}
