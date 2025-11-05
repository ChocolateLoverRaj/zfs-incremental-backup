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
    env::current_dir,
    io::{self, ErrorKind},
    num::NonZero,
    path::PathBuf,
};

use aws_config::Region;
use aws_sdk_s3::{config::Credentials, types::StorageClass};
use rcs3ud::{
    AmountLimiter2, NoOpAmountLimiter2, NoOpOperationScheduler2, OperationScheduler2, S3Dest,
};
use tokio::fs::{OpenOptions, read_to_string, remove_file, write};

use crate::{
    backup::backup, zfs_create::zfs_create, zfs_dataset::ZfsDataset, zfs_snapshot::ZfsSnapshot,
    zpool_create::zpool_create, zpool_ensure_destroy::zpool_ensure_destroy,
};

#[tokio::main]
async fn main() {
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
    // client.create_bucket().bucket("test").send().await.unwrap();
    let a = client.list_buckets().send().await.unwrap();
    let buckets = a.buckets();
    println!("{buckets:?}");

    let zpool = "zfs-incremental-backup-dev";
    let output = zpool_ensure_destroy(zpool).await.unwrap();
    println!("{output:?}");

    let zpool_file_path = "./dev/zpool";
    let zpool_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(zpool_file_path)
        .await
        .unwrap();
    zpool_file.set_len(64 * 1024 * 1024).await.unwrap();
    zpool_create(
        zpool,
        current_dir()
            .unwrap()
            .join(zpool_file_path)
            .to_str()
            .unwrap(),
    )
    .await
    .unwrap();
    let dataset = "test";
    zfs_create(ZfsDataset {
        zpool: zpool.into(),
        dataset: dataset.into(),
    })
    .await
    .unwrap();

    let backup0_snapshot = ZfsSnapshot {
        zpool: zpool.into(),
        dataset: dataset.into(),
        snapshot_name: "backup0".into(),
    };

    #[allow(unused)]
    #[derive(Debug)]
    enum SaveError {
        Serialize(ron::Error),
        Write(io::Error),
    }
    let chunk_size = NonZero::new(30_000).unwrap();
    let backup_save_file = "./dev/save_data_backup0.ron";
    backup(
        match read_to_string(backup_save_file).await {
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
        backup0_snapshot.clone(),
        None,
        &PathBuf::from("./dev/backup0"),
        S3Dest {
            bucket: "test",
            object_key: "backup0",
            storage_class: StorageClass::Standard,
        },
        &client,
        &mut (Box::new(NoOpAmountLimiter2)
            as Box<dyn AmountLimiter2<ReserveError = (), MarkUsedError = ()> + Send>),
        &mut (Box::new(NoOpOperationScheduler2) as Box<dyn OperationScheduler2 + Send>),
        chunk_size,
        &mut async |save_data| {
            write(
                &backup_save_file,
                ron::to_string(save_data).map_err(SaveError::Serialize)?,
            )
            .await
            .map_err(SaveError::Write)?;
            Ok::<_, SaveError>(())
        },
    )
    .await
    .unwrap();
    println!("Backed up snapshot backup0");
    remove_file(backup_save_file).await.unwrap();

    let backup_save_file = "./dev/save_data_backup0_backup1.ron";
    backup(
        match read_to_string(backup_save_file).await {
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
            zpool: zpool.into(),
            dataset: dataset.into(),
            snapshot_name: "backup1".into(),
        },
        Some(backup0_snapshot.snapshot_name.clone()),
        &PathBuf::from("./dev/backup0_backup1"),
        S3Dest {
            bucket: "test",
            object_key: "backup0_backup1",
            storage_class: StorageClass::Standard,
        },
        &client,
        &mut (Box::new(NoOpAmountLimiter2)
            as Box<dyn AmountLimiter2<ReserveError = (), MarkUsedError = ()> + Send>),
        &mut (Box::new(NoOpOperationScheduler2) as Box<dyn OperationScheduler2 + Send>),
        chunk_size,
        &mut async |save_data| {
            write(
                &backup_save_file,
                ron::to_string(save_data).map_err(SaveError::Serialize)?,
            )
            .await
            .map_err(SaveError::Write)?;
            Ok::<_, SaveError>(())
        },
    )
    .await
    .unwrap();
    println!("Backed up incremental snapshot from backup0 to backup1");
    remove_file(backup_save_file).await.unwrap();
}
