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
    let zfs_snapshot = ZfsSnapshot {
        zpool: "zfs-incremental-backup-dev".into(),
        dataset: "test".into(),
        snapshot_name: "backup0".into(),
    };
    let output = zfs_ensure_snapshot(zfs_snapshot.clone()).await.unwrap();
    println!("{output:#?}");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open("dev/backup0")
        .unwrap();
    zfs_send(zfs_snapshot, file.into()).await.unwrap();
}
