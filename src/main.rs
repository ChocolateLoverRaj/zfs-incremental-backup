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

use std::env::current_dir;

use tokio::fs::OpenOptions;

use crate::{
    zfs_create::zfs_create, zfs_dataset::ZfsDataset, zfs_ensure_snapshot::zfs_ensure_snapshot,
    zfs_send::zfs_send, zfs_snapshot::ZfsSnapshot, zpool_create::zpool_create,
    zpool_ensure_destroy::zpool_ensure_destroy,
};

#[tokio::main]
async fn main() {
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
    let output = zfs_ensure_snapshot(backup0_snapshot.clone()).await.unwrap();
    println!("{output:#?}");
    zfs_send(
        backup0_snapshot,
        None,
        OpenOptions::new()
            .create(true)
            .write(true)
            .open("dev/backup0")
            .await
            .unwrap()
            .into_std()
            .await
            .into(),
    )
    .await
    .unwrap();

    let backup1_snapshot_name = "backup1";
    let backup1_snapshot = ZfsSnapshot {
        zpool: zpool.into(),
        dataset: dataset.into(),
        snapshot_name: backup1_snapshot_name.into(),
    };
    let output = zfs_ensure_snapshot(backup1_snapshot.clone()).await.unwrap();
    println!("{output:#?}");
    zfs_send(
        backup1_snapshot,
        None,
        OpenOptions::new()
            .create(true)
            .write(true)
            .open("dev/backup0_backup1")
            .await
            .unwrap()
            .into_std()
            .await
            .into(),
    )
    .await
    .unwrap();
}
