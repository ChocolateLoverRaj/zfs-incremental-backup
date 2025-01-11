use aes_gcm::{aead::Aead, Aes256Gcm, Key, KeyInit, Nonce};
use anyhow::anyhow;
use argon2::{password_hash::Salt, Argon2};
use rand::{thread_rng, Rng};

/// Create an encryption key based on a password
pub fn derive_key(password: &[u8], salt: &[u8]) -> anyhow::Result<Key<Aes256Gcm>> {
    let mut key = Key::<Aes256Gcm>::default();
    Argon2::default()
        .hash_password_into(password, salt, key.as_mut_slice())
        .map_err(|e| anyhow!("Failed to create key: {e:?}"))?;
    Ok(key)
}

pub fn generate_salt_and_derive_key(
    password: &[u8],
) -> anyhow::Result<([u8; Salt::RECOMMENDED_LENGTH], Key<Aes256Gcm>)> {
    let salt = thread_rng().gen::<[u8; Salt::RECOMMENDED_LENGTH]>();
    let key = derive_key(&password, &salt)?;
    Ok((salt, key))
}

pub fn encrypt_immutable_key(
    password_derived_key: &Key<Aes256Gcm>,
    unencrypted_immutable_key: &[u8],
) -> anyhow::Result<[u8; 32 + 16]> {
    let cipher = Aes256Gcm::new(password_derived_key);
    let nonce = Nonce::default();
    let encrypted_immutable_key = cipher
        .encrypt(&nonce, unencrypted_immutable_key)
        .map_err(|e| anyhow!("Failed to encrypt: {e:?}"))?
        .try_into()
        // It should always be the right size
        .unwrap();
    Ok(encrypted_immutable_key)
}
