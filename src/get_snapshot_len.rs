use aws_smithy_types_convert::stream::PaginationStreamImplStream;
use futures::TryStreamExt;

use crate::{
    backup_config::BackupConfig, backup_data::BackupData, config::SNAPSHOTS_PREFIX,
    get_encrypted_snapshot_name::get_encrypted_snapshot_name,
    remote_hot_data::RemoteHotDataInMemory,
};

/// Get the size of the snapshot
pub async fn get_snapshot_len<'a>(
    s3_client: &aws_sdk_s3::Client,
    config: &'a BackupConfig,
    data: BackupData<'a>,
    remote_hot_data: RemoteHotDataInMemory<'a>,
    snapshot_name: &'a str,
) -> anyhow::Result<u64> {
    Ok({
        let snapshots_folder = {
            let snapshot_name =
                get_encrypted_snapshot_name(config, remote_hot_data, snapshot_name).await?;
            format!("{}/{}", SNAPSHOTS_PREFIX, snapshot_name)
        };
        let len = PaginationStreamImplStream::new(
            s3_client
                .list_objects_v2()
                .bucket(data.s3_bucket)
                .prefix(snapshots_folder)
                .into_paginator()
                .send(),
        )
        .try_fold(0, |len, item| async move {
            Ok(item.contents().iter().fold(len, |len, object| {
                len + (object.size.unwrap_or_default() as u64)
            }))
        })
        .await?;
        len
    })
}
