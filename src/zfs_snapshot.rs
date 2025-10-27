#[derive(Debug, Clone)]
pub struct ZfsSnapshot {
    pub zpool: String,
    pub dataset: String,
    pub snapshot_name: String,
}

impl ToString for ZfsSnapshot {
    fn to_string(&self) -> String {
        let Self {
            zpool,
            dataset,
            snapshot_name,
        } = self;
        format!("{zpool}/{dataset}@{snapshot_name}")
    }
}
