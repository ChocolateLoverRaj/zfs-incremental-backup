use rand::random;

use crate::{
    create_immutable_key::create_immutable_key,
    derive_key::{encrypt_immutable_key, generate_salt_and_derive_key},
    remote_hot_data::EncryptionData,
};

pub fn init_encryption_data(password: &[u8]) -> anyhow::Result<EncryptionData> {
    Ok({
        let (salt, key) = generate_salt_and_derive_key(password).unwrap();
        EncryptionData {
            encrypted_root_key: { encrypt_immutable_key(&key, &create_immutable_key())? },
            password_derived_key_salt: salt,
            blake3_salt: random(),
            aes_256_gcm_salt: random(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::init_encryption_data;

    #[test]
    fn ok() {
        let encryption_data = init_encryption_data(b"password").unwrap();
        println!("{:#?}", encryption_data);
    }
}
