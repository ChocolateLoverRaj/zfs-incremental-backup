use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

use crate::{auto_back::AutoBackupState, zfs_dataset::ZfsDataset};

/// Configuration that should not change for the lifetime of this file, unless you change the zpool / dataset name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBackupConfig {
    pub dataset: ZfsDataset,
    pub bucket: String,
    pub snapshot_prefix: String,
    pub object_prefix: String,
}

/// The config and state are in the same file so that the user doesn't accidentally specify the wrong config and state
#[derive(Debug, Serialize, Deserialize)]
pub struct AutoBackupFileData {
    pub config: AutoBackupConfig,
    pub state: AutoBackupState,
}

#[derive(Debug, Parser)]
pub struct Cli {
    #[arg(long)]
    zpool: String,
    #[arg(long)]
    dataset: String,
    /// For example, if this is `backup`, then snapshots will be called `backup0`, `backup1`, etc.
    /// Incremental backups will be separated by `_`, so they will be called `backup0_backup1`, `backup1_backup2`, etc.
    /// `backup0_backup1` means that the "file" contains the data to create @backup1 if you already have @backup0
    #[arg(long)]
    snapshot_prefix: String,
    /// The S3 bucket to upload to
    #[arg(long)]
    bucket: String,
    /// The prefix to upload S3 objects to
    #[arg(long)]
    object_prefix: String,
    /// A path where a single file will be saved that keeps track of the state of this program, including the last uploaded snapshot and backup progress.
    #[arg(long)]
    save_data_path: String,
}

pub async fn init_auto_back(
    Cli {
        zpool,
        dataset,
        snapshot_prefix,
        bucket,
        object_prefix,
        save_data_path,
    }: Cli,
) {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(save_data_path)
        .await
        .unwrap()
        .write_all(
            ron::ser::to_string_pretty(
                &AutoBackupFileData {
                    config: AutoBackupConfig {
                        dataset: ZfsDataset { zpool, dataset },
                        snapshot_prefix,
                        object_prefix,
                        bucket,
                    },
                    state: Default::default(),
                },
                Default::default(),
            )
            .unwrap()
            .as_bytes(),
        )
        .await
        .unwrap();
}
