pub struct ZfsSnapshotInput {
    pub zpool: String,
    pub dataset: String,
    pub snapshot_name: String,
}

pub fn zfs_ensure_snapshot(input: ZfsSnapshotInput) -> ZpoolResult<()> {
    let zfs = ZpoolOpen3::default();
    println!(zfs.exists("test")?);
    todo!()
}
