use crate::zfs_snapshot::{ZfsSnapshotInput, zfs_ensure_snapshot};

mod zfs_snapshot;

fn main() {
    zfs_ensure_snapshot(ZfsSnapshotInput {
        zpool: "test".into(),
        dataset: "test".into(),
        snapshot_name: "backup0",
    })
    .unwrap();
}
