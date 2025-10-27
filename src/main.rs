mod zfs_ensure_snapshot;
mod zfs_send;
mod zfs_snapshot;
mod zfs_snapshot_exists;
mod zfs_take_snapshot;

use crate::{
    zfs_ensure_snapshot::zfs_ensure_snapshot, zfs_send::zfs_send, zfs_snapshot::ZfsSnapshot,
};
use std::fs::OpenOptions;

#[tokio::main]
async fn main() {
    let zpool = "zfs-incremental-backup-dev";
    let dataset = "test";
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
            .unwrap()
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
            .unwrap()
            .into(),
    )
    .await
    .unwrap();
}
