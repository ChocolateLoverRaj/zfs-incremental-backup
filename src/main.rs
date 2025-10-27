mod zfs_ensure_snapshot;
mod zfs_snapshot_exists;
mod zfs_take_snapshot;

use crate::{zfs_ensure_snapshot::zfs_ensure_snapshot, zfs_take_snapshot::ZfsSnapshot};

#[tokio::main]
async fn main() {
    let output = zfs_ensure_snapshot(ZfsSnapshot {
        zpool: "zfs-incremental-backup-dev".into(),
        dataset: "test".into(),
        snapshot_name: "backup0".into(),
    })
    .await
    .unwrap();
    println!("{output:#?}");
}
