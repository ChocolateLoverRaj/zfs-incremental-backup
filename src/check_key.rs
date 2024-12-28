use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use anyhow::anyhow;

use crate::{backup_data::EncryptionData, derive_key::derive_key};

pub fn decrypt_immutable_key(
    encryption_password: &[u8],
    encryption_data: &EncryptionData,
) -> anyhow::Result<Vec<u8>> {
    let key = derive_key(
        &encryption_password,
        &encryption_data.password_derived_key_salt,
    )?;
    let cipher = Aes256Gcm::new(&key);
    let decrypted_key = cipher
        .decrypt(
            &Nonce::default(),
            encryption_data.encrypted_immutable_key.as_ref(),
        )
        .map_err(|e| anyhow!("Failed to decrypt encrypted immutable key: {e:?}"))?;
    Ok(decrypted_key)
}
