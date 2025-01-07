use std::borrow::Cow;
use std::fmt::Display;

use aes_gcm::{Aes256Gcm, Key};
use aws_sdk_s3::types::BucketLocationConstraint;
use rand::{thread_rng, Rng};

use crate::backup_data::BackupData;
use crate::create_bucket::create_bucket;
use crate::derive_key::{encrypt_immutable_key, generate_salt_and_derive_key};
use crate::remote_hot_data::{upload_hot_data, EncryptionData, RemoteHotData};

pub async fn init<'a>(
    s3_client: &aws_sdk_s3::Client,
    bucket_prefix: &'a impl Display,
    location: &BucketLocationConstraint,
    encryption_password: &Option<Vec<u8>>,
) -> anyhow::Result<BackupData<'a>> {
    let bucket = create_bucket(s3_client, bucket_prefix, location).await?;

    let encryption_data =
        encryption_password
            .as_ref()
            .map_or(anyhow::Ok(None), |encryption_password| {
                // println!("Encryption password: {:?}", encryption_password);

                // We will create an encryption key randomly
                let immutable_key = {
                    let mut immutable_key = Key::<Aes256Gcm>::default();
                    thread_rng().fill(immutable_key.as_mut_slice());
                    immutable_key
                };
                // println!("Immutable key: {:?}", immutable_key);
                // We will also create a key derived from the password, along with a random salt
                let (password_derived_key_salt, password_derived_key) =
                    generate_salt_and_derive_key(&encryption_password)?;
                // println!("password_derived_key_salt: {:?}", password_derived_key_salt);
                // println!("password_derived_key: {:?}", password_derived_key);
                // We will then encrypt the encryption key itself using the password
                let encrypted_immutable_key =
                    encrypt_immutable_key(&password_derived_key, immutable_key.as_slice())?;
                // println!("encrypted_immutable_key: {:?}", encrypted_immutable_key);
                Ok(Some(EncryptionData {
                    password_derived_key_salt,
                    encrypted_immutable_key,
                }))
            })?;

    let backup_data = BackupData {
        s3_bucket: Cow::Owned(bucket),
        s3_region: Cow::Owned(location.to_string()),
        last_saved_snapshot_name: None,
        backup_step: None,
    };

    upload_hot_data(
        s3_client,
        &backup_data.s3_bucket,
        &RemoteHotData {
            encryption: encryption_data,
            snapshots: Default::default(),
        },
    )
    .await?;

    Ok(backup_data)
}
