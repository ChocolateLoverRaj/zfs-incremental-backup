use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZfsDataset {
    pub zpool: String,
    pub dataset: String,
}

impl ToString for ZfsDataset {
    fn to_string(&self) -> String {
        let Self { zpool, dataset } = self;
        format!("{zpool}/{dataset}")
    }
}
