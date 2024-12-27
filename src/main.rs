use std::fmt::Display;
use std::io::ErrorKind;
use std::path::PathBuf;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use anyhow::anyhow;
use argon2::password_hash::Salt;
use argon2::Argon2;
use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{BucketLocationConstraint, StorageClass};
use clap::{Parser, Subcommand};
use create_bucket::create_bucket;
use encryption_password::EncryptionPassword;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use tokio::fs::{read_to_string, OpenOptions};
use tokio::io::AsyncWriteExt;

mod create_bucket;
mod encryption_password;

#[derive(Debug, Serialize, Deserialize)]
struct EncryptionData {
    password_derived_key_salt: [u8; Salt::RECOMMENDED_LENGTH],
    encrypted_immutable_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupData {
    s3_bucket: String,
    encryption: Option<EncryptionData>,
}

async fn init(
    s3_client: &aws_sdk_s3::Client,
    bucket_prefix: &impl Display,
    location: &BucketLocationConstraint,
    encryption_password: &Option<Vec<u8>>,
) -> anyhow::Result<BackupData> {
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
                let (password_derived_key_salt, password_derived_key) = {
                    let mut key = Key::<Aes256Gcm>::default();
                    let salt = thread_rng().gen::<[u8; Salt::RECOMMENDED_LENGTH]>();
                    Argon2::default()
                        .hash_password_into(&encryption_password, &salt, key.as_mut_slice())
                        .map_err(|e| anyhow!("Failed to create key: {e:?}"))?;
                    (salt, key)
                };
                // println!("password_derived_key_salt: {:?}", password_derived_key_salt);
                // println!("password_derived_key: {:?}", password_derived_key);
                // We will then encrypt the encryption key itself using the password
                let encrypted_immutable_key = {
                    let cipher = Aes256Gcm::new(&password_derived_key);
                    let nonce = Nonce::default();
                    cipher
                        .encrypt(&nonce, immutable_key.as_slice())
                        .map_err(|e| anyhow!("Failed to encrypt: {e:?}"))?
                };
                // println!("encrypted_immutable_key: {:?}", encrypted_immutable_key);
                Ok(Some(EncryptionData {
                    password_derived_key_salt,
                    encrypted_immutable_key,
                }))
            })?;

    if let Some(encryption_data) = &encryption_data {
        s3_client
            .put_object()
            .bucket(&bucket)
            .key("encryption_data")
            .body(ByteStream::from(postcard::to_allocvec(encryption_data)?))
            .storage_class(StorageClass::Standard)
            .send()
            .await?;
    }

    Ok(BackupData {
        s3_bucket: bucket,
        encryption: encryption_data,
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct BackupConfig {
    /// You can change the encryption password later, but you can't change from Some to None or None to Some.
    /// You can set the encryption password to an empty string to be able to set a password later.
    encryption_password: Option<EncryptionPassword>,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// does testing things
    Init {
        /// The bucket name will be the prefix with a GUID. One example of a bucket name is ``. Must follow https://docs.aws.amazon.com/AmazonS3/latest/userguide/bucketnamingrules.html or you will get an error.
        #[arg(short, long, default_value = "zfs-backup")]
        bucket_prefix: String,
        #[arg(short, long, default_value = "us-west-2")]
        region: BucketLocationConstraint,
        /// Path to a JSON file with config
        #[arg(short, long)]
        config_path: PathBuf,
        /// Path to the backup data JSON file
        #[arg(short, long)]
        data_path: PathBuf,
        /// Overwrite the data file
        #[arg(short, long)]
        force: bool,
    },
    Backup {
        /// Path to a JSON file with config
        #[arg(short, long)]
        config_path: PathBuf,
        /// Path to the backup data JSON file
        #[arg(short, long)]
        data_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            bucket_prefix,
            region,
            config_path,
            data_path,
            force,
        } => {
            let mut backup_data_file = OpenOptions::new()
                .read(false)
                .write(true)
                .create(true)
                .create_new(!force)
                .open(data_path)
                .await
                .map_err(|e| match e.kind() {
                    ErrorKind::AlreadyExists => anyhow::Error::from(e)
                        .context("Backup data file already exists. Use -f to overwrite."),
                    _ => e.into(),
                })?;
            let config = {
                let config = read_to_string(config_path).await?;
                let config = serde_json::from_str::<BackupConfig>(&config)?;
                config
            };
            let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
            let s3_client = aws_sdk_s3::Client::new(&sdk_config);
            let backup_data = init(
                &s3_client,
                &bucket_prefix,
                &region,
                &match &config.encryption_password {
                    Some(encryption_password) => Some(encryption_password.get_bytes().await?),
                    None => None,
                },
            )
            .await?;
            backup_data_file
                .write_all(serde_json::to_string_pretty(&backup_data)?.as_bytes())
                .await?;
            println!("Saved backup data: {:#?}", backup_data);
        }
        Commands::Backup {
            config_path,
            data_path,
        } => {}
    }
    Ok(())
}
