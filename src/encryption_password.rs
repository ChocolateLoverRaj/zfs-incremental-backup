use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::{fs::File, io::AsyncReadExt};

/// Basically a `Vec<u8>` which is used to encrypt and decrypt the data. If you set an encryption password you can change it later.
#[derive(Debug, Serialize, Deserialize)]
pub enum EncryptionPassword {
    /// Uses the bytes of the string as the encryption password. If you use this then your config file is a secret.
    Plain(String),
    /// Parses the hex bytes from the string. If you use this then your config file is a secret.
    Hex(String),
    /// Read from a file which contains the key. This way you can keep your config public while keeping the key file a secret.
    File(PathBuf),
}

impl EncryptionPassword {
    pub async fn get_bytes(&self) -> anyhow::Result<Vec<u8>> {
        match self {
            Self::Plain(string) => Ok(string.as_bytes().to_vec()),
            Self::Hex(hex_string) => Ok(hex::decode(hex_string)?),
            Self::File(password_path) => {
                let mut password = Default::default();
                File::open(password_path)
                    .await?
                    .read_to_end(&mut password)
                    .await?;
                Ok(password)
            }
        }
    }
}
