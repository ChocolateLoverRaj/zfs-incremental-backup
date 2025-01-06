// We need to store some data as hot data. For now we will just store it as a S3 Standard object. You could store this in a database or DynamoDB or something.

use argon2::password_hash::Salt;
use aws_sdk_s3::{primitives::ByteStream, types::StorageClass};
use serde::{Deserialize, Serialize};

use crate::{backup_data::BackupData, config::HOT_DATA_OBJECT_KEY};

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptionData {
    pub password_derived_key_salt: [u8; Salt::RECOMMENDED_LENGTH],
    pub encrypted_immutable_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteHotData {
    pub encryption: Option<EncryptionData>,
    /// The names of the snapshots, as they appear as a S3 key.
    /// For example, a snapshot might be stored in the S3 objects `snapshots/<snapshot_name>/0`, `snapshots/<snapshot_name>/1`
    pub snapshots: Vec<String>,
}

pub async fn upload_hot_data(
    s3_client: &aws_sdk_s3::Client,
    s3_bucket: &str,
    remote_hot_data: &RemoteHotData,
) -> anyhow::Result<()> {
    s3_client
        .put_object()
        .bucket(s3_bucket)
        .key(HOT_DATA_OBJECT_KEY)
        .body(ByteStream::from(postcard::to_allocvec(remote_hot_data)?))
        .storage_class(StorageClass::Standard)
        .send()
        .await?;
    Ok(())
}

pub async fn download_hot_data(
    s3_client: &aws_sdk_s3::Client,
    s3_bucket: &str,
) -> anyhow::Result<RemoteHotData> {
    let remote_hot_data = s3_client
        .get_object()
        .bucket(s3_bucket)
        .key(HOT_DATA_OBJECT_KEY)
        .send()
        .await?
        .body
        .collect()
        .await?
        .into_bytes();
    let s3_encryption_data = postcard::from_bytes(&remote_hot_data)?;
    Ok(s3_encryption_data)
}
