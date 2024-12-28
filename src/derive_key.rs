use aes_gcm::{Aes256Gcm, Key};
use anyhow::anyhow;
use argon2::Argon2;

/// Create an encryption key based on a password
pub fn derive_key(password: &[u8], salt: &[u8]) -> anyhow::Result<Key<Aes256Gcm>> {
    let mut key = Key::<Aes256Gcm>::default();
    Argon2::default()
        .hash_password_into(password, salt, key.as_mut_slice())
        .map_err(|e| anyhow!("Failed to create key: {e:?}"))?;
    Ok(key)
}
