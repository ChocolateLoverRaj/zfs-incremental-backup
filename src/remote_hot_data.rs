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

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
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

/// This data may be encrypted, depending on config
#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub struct RemoteHotEncryptedData<'a> {
    pub snapshots: Snapshots<'a>,
    pub sqs: Cow<'a, str>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub struct RemoteHotDataEncrypted<'a> {
    pub encryption_data: Cow<'a, EncryptionData>,
    pub encrypted_data: Cow<'a, Vec<u8>>,
}

impl<'a> RemoteHotDataEncrypted<'a> {
    pub fn decrypt(
        mut self,
        encryption_password: &[u8],
    ) -> anyhow::Result<(Cow<'a, EncryptionData>, RemoteHotEncryptedData<'a>)> {
        Ok({
            let immutable_key = decrypt_immutable_key(encryption_password, &self.encryption_data)?;
            let mut cipher = Aes256Gcm::new_from_slice(&immutable_key)?;
            let buffer = self.encrypted_data.to_mut();
            cipher
                .decrypt_in_place(&Nonce::default(), &[], buffer)
                .map_err(|e| anyhow!("Failed to decrypt snapshots: {e:?}"))?;
            (self.encryption_data, postcard::from_bytes(&buffer)?)
        })
    }
}

pub type Snapshots<'a> = Vec<Cow<'a, str>>;

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub enum RemoteHotData<'a> {
    NotEncrypted(RemoteHotEncryptedData<'a>),
    Encrypted(RemoteHotDataEncrypted<'a>),
}

impl<'a> RemoteHotData<'a> {
    pub fn decrypt(
        self,
        encryption_password: Option<&[u8]>,
    ) -> anyhow::Result<RemoteHotDataInMemory<'a>> {
        Ok(match self {
            RemoteHotData::NotEncrypted(data) => RemoteHotDataInMemory {
                encryption: None,
                data,
            },
            RemoteHotData::Encrypted(encrypted) => {
                let (encryption_data, data) = encrypted
                    .decrypt(encryption_password.ok_or(anyhow!("Expected encryption password"))?)?;
                RemoteHotDataInMemory {
                    data,
                    encryption: Some(encryption_data),
                }
            }
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, ShallowClone)]
pub struct RemoteHotDataInMemory<'a> {
    pub encryption: Option<Cow<'a, EncryptionData>>,
    pub data: RemoteHotEncryptedData<'a>,
}

impl<'a> RemoteHotDataInMemory<'a> {
    fn encrypt(self, encryption_password: Option<&[u8]>) -> anyhow::Result<RemoteHotData<'a>> {
        Ok(match self.encryption {
            None => RemoteHotData::NotEncrypted(self.data),
            Some(encryption) => RemoteHotData::Encrypted(RemoteHotDataEncrypted {
                encrypted_data: Cow::Owned({
                    let immutable_key = decrypt_immutable_key(
                        encryption_password.ok_or(anyhow!("Expected encryption password"))?,
                        &encryption,
                    )?;
                    let mut cipher = Aes256Gcm::new_from_slice(&immutable_key)?;
                    let mut buffer = postcard::to_allocvec(&self.data)?;
                    postcard::from_bytes::<RemoteHotEncryptedData<'_>>(&buffer).unwrap();
                    cipher
                        .encrypt_in_place(&Nonce::default(), &[], &mut buffer)
                        .map_err(|e| anyhow!("Failed to encrypt snapshots: {e:?}"))?;
                    buffer
                }),
                encryption_data: encryption,
            }),
        })
    }
}

pub async fn upload_hot_data<'a>(
    config: &BackupConfig,
    s3_client: &aws_sdk_s3::Client,
    s3_bucket: &str,
    remote_hot_data: RemoteHotDataInMemory<'a>,
) -> anyhow::Result<()> {
    s3_client
        .put_object()
        .bucket(s3_bucket)
        .key(HOT_DATA_OBJECT_KEY)
        .body(ByteStream::from(postcard::to_allocvec(
            &remote_hot_data.encrypt(
                match &config.encryption {
                    None => None,
                    Some(encryption) => Some(encryption.password.get_bytes().await?),
                }
                .as_ref()
                .map(|vec| vec.as_slice()),
            )?,
        )?))
        .storage_class(StorageClass::Standard)
        .send()
        .await?;
    Ok(())
}

pub async fn download_hot_data_encrypted(
    s3_client: &aws_sdk_s3::Client,
    s3_bucket: &str,
) -> anyhow::Result<RemoteHotData<'static>> {
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
    let s3_encryption_data = postcard::from_bytes::<RemoteHotData>(&remote_hot_data)?;
    Ok(s3_encryption_data)
}

pub async fn download_hot_data(
    config: &BackupConfig,
    s3_client: &aws_sdk_s3::Client,
    s3_bucket: &str,
) -> anyhow::Result<RemoteHotDataInMemory<'static>> {
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
    let s3_encryption_data = postcard::from_bytes::<RemoteHotData>(&remote_hot_data)?.decrypt(
        match &config.encryption {
            None => None,
            Some(encryption) => Some(encryption.password.get_bytes().await?),
        }
        .as_ref()
        .map(|vec| vec.as_slice()),
    )?;
    Ok(s3_encryption_data)
}
