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
    path::PathBuf,
    str::FromStr,
};

use aws_config::{BehaviorVersion, Region, meta::region::RegionProviderChain};
use aws_sdk_s3::{
    Client,
    config::{Credentials, endpoint::Endpoint},
    primitives::ByteStream,
};
use tokio::fs::{OpenOptions, read_to_string, write};

use crate::{
    backup::{BackupCallbacks, BackupSaveData, backup},
    zfs_create::zfs_create,
    zfs_dataset::ZfsDataset,
    zfs_snapshot::ZfsSnapshot,
    zpool_create::zpool_create,
    zpool_ensure_destroy::zpool_ensure_destroy,
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

    let backup_save_file = "./dev/save_data.ron";
    let save_data = match read_to_string(backup_save_file).await {
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
    };
    #[derive(Debug)]
    struct MyBackupCallbacks(PathBuf);
    #[derive(Debug)]
    enum SaveError {
        Serialize(ron::Error),
        Write(io::Error),
    }
    impl BackupCallbacks for MyBackupCallbacks {
        type SaveError = SaveError;

        async fn save(&mut self, data: &BackupSaveData) -> Result<(), Self::SaveError> {
            write(&self.0, ron::to_string(data).map_err(SaveError::Serialize)?)
                .await
                .map_err(SaveError::Write)?;
            Ok(())
        }
    }
    backup(
        save_data,
        backup0_snapshot,
        None,
        "./dev/backup0".into(),
        &mut MyBackupCallbacks(backup_save_file.into()),
    )
    .await
    .unwrap();

    // let output = zfs_ensure_snapshot(backup0_snapshot.clone()).await.unwrap();
    // println!("{output:#?}");
    // zfs_send(
    //     backup0_snapshot,
    //     None,
    //     OpenOptions::new()
    //         .create(true)
    //         .write(true)
    //         .open("dev/backup0")
    //         .await
    //         .unwrap()
    //         .into_std()
    //         .await
    //         .into(),
    // )
    // .await
    // .unwrap();

    // let backup1_snapshot_name = "backup1";
    // let backup1_snapshot = ZfsSnapshot {
    //     zpool: zpool.into(),
    //     dataset: dataset.into(),
    //     snapshot_name: backup1_snapshot_name.into(),
    // };
    // let output = zfs_ensure_snapshot(backup1_snapshot.clone()).await.unwrap();
    // println!("{output:#?}");
    // zfs_send(
    //     backup1_snapshot,
    //     None,
    //     OpenOptions::new()
    //         .create(true)
    //         .write(true)
    //         .open("dev/backup0_backup1")
    //         .await
    //         .unwrap()
    //         .into_std()
    //         .await
    //         .into(),
    // )
    // .await
    // .unwrap();
}
