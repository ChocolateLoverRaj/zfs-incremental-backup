#[derive(Debug, Clone, Copy)]
pub struct ZfsSnapshot<'a> {
    pub zpool: &'a str,
    pub dataset: &'a str,
    pub snapshot_name: &'a str,
}

impl ToString for ZfsSnapshot<'_> {
    fn to_string(&self) -> String {
        let Self {
            zpool,
            dataset,
            snapshot_name,
        } = self;
        format!("{zpool}/{dataset}@{snapshot_name}")
    }
}
