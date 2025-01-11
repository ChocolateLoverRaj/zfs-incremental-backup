// We need to store some data as hot data. For now we will just store it as a S3 Standard object. You could store this in a database or DynamoDB or something.

use std::borrow::Cow;

use crate::{
    backup_config::BackupConfig, config::HOT_DATA_OBJECT_KEY,
    decrypt_immutable_key::decrypt_immutable_key,
};
use aead::{AeadMutInPlace, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context};
use argon2::password_hash::Salt;
use aws_sdk_s3::{primitives::ByteStream, types::StorageClass};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use shallowclone::ShallowClone;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptionData {
    pub password_derived_key_salt: [u8; Salt::RECOMMENDED_LENGTH],
    /// 32 bytes for the key, plus 16 bytes for aes-256-gcm tag
    #[serde(with = "BigArray")]
    pub encrypted_root_key: [u8; 32 + 16],
    /// Used to derive an aes-256-gcm key from the root key
    pub aes_256_gcm_salt: [u8; Salt::RECOMMENDED_LENGTH],
    /// Used to derive a blake3 key from the root key
    pub blake3_salt: [u8; Salt::RECOMMENDED_LENGTH],
}

pub type Snapshots<'a> = Vec<Cow<'a, str>>;

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
struct RemoteHotData<'a> {
    pub encryption: Option<Cow<'a, EncryptionData>>,
    /// Snapshots may be encrypted, depending on options
    pub snapshots: Vec<u8>,
}

impl<'a> RemoteHotData<'a> {
    async fn decrypt(
        mut self,
        config: &BackupConfig,
    ) -> anyhow::Result<RemoteHotDataDecrypted<'a>> {
        Ok(RemoteHotDataDecrypted {
            snapshots: {
                let decrypted_bytes = match &self.encryption {
                    None => self.snapshots,
                    Some(encryption_data) => {
                        let encryption_config = config.encryption.as_ref().unwrap();
                        if encryption_config.encrypt_snapshot_names {
                            let immutable_key = decrypt_immutable_key(
                                &encryption_config.password.get_bytes().await?,
                                &encryption_data,
                            )?;
                            let mut cipher = Aes256Gcm::new_from_slice(&immutable_key)?;
                            cipher
                                .decrypt_in_place(&Nonce::default(), &[], &mut self.snapshots)
                                .map_err(|e| anyhow!("Failed to decrypt snapshots: {e:?}"))?;
                        }
                        self.snapshots
                    }
                };
                postcard::from_bytes(&decrypted_bytes)?
            },
            encryption: self.encryption,
        })
    }
}

/// With decrypted snapshots
#[derive(Debug, Clone, ShallowClone)]
pub struct RemoteHotDataDecrypted<'a> {
    pub encryption: Option<Cow<'a, EncryptionData>>,
    /// The names of the snapshots, as they appear as a S3 key.
    /// For example, a snapshot might be stored in the S3 objects `snapshots/<snapshot_name>/0`, `snapshots/<snapshot_name>/1`
    pub snapshots: Snapshots<'a>,
}

impl<'a> RemoteHotDataDecrypted<'a> {
    async fn encrypt(&'a self, config: &BackupConfig) -> anyhow::Result<RemoteHotData<'a>> {
        Ok(RemoteHotData {
            encryption: self.encryption.shallow_clone(),
            snapshots: {
                let mut snapshots = postcard::to_allocvec(&self.snapshots)?;
                match &self.encryption {
                    None => snapshots,
                    Some(encryption) => {
                        let encryption_config = config.encryption.as_ref().unwrap();
                        if encryption_config.encrypt_snapshot_names {
                            let immutable_key = decrypt_immutable_key(
                                &encryption_config.password.get_bytes().await?,
                                &encryption,
                            )?;
                            let mut cipher = Aes256Gcm::new_from_slice(&immutable_key)?;
                            cipher
                                .encrypt_in_place(&Nonce::default(), &[], &mut snapshots)
                                .map_err(|e| anyhow!("Failed to encrypt snapshots: {e:?}"))?;
                        }
                        snapshots
                    }
                }
            },
        })
    }
}

pub async fn upload_hot_data<'a>(
    config: &BackupConfig,
    s3_client: &aws_sdk_s3::Client,
    s3_bucket: &str,
    remote_hot_data: &RemoteHotDataDecrypted<'a>,
) -> anyhow::Result<()> {
    s3_client
        .put_object()
        .bucket(s3_bucket)
        .key(HOT_DATA_OBJECT_KEY)
        .body(ByteStream::from(postcard::to_allocvec(
            &remote_hot_data.encrypt(config).await?,
        )?))
        .storage_class(StorageClass::Standard)
        .send()
        .await?;
    Ok(())
}

pub async fn download_hot_data(
    config: &BackupConfig,
    s3_client: &aws_sdk_s3::Client,
    s3_bucket: &str,
) -> anyhow::Result<RemoteHotDataDecrypted<'static>> {
    let remote_hot_data = s3_client
        .get_object()
        .bucket(s3_bucket)
        .key(HOT_DATA_OBJECT_KEY)
        .send()
        .await
        .context("Failed to send hot data download request")?
        .body
        .collect()
        .await
        .context("Failed to download hot data")?
        .into_bytes();
    let s3_encryption_data = postcard::from_bytes::<RemoteHotData>(&remote_hot_data)?
        .decrypt(&config)
        .await?;
    Ok(s3_encryption_data)
}
