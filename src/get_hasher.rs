use anyhow::anyhow;
use argon2::Argon2;

use crate::{decrypt_immutable_key::decrypt_immutable_key, remote_hot_data::EncryptionData};

pub fn get_hasher(
    encryption_password: &[u8],
    encryption_data: &EncryptionData,
) -> anyhow::Result<blake3::Hasher> {
    Ok({
        let derived_key = {
            let mut derived_key: [_; blake3::KEY_LEN] = Default::default();
            Argon2::default()
                .hash_password_into(
                    &decrypt_immutable_key(encryption_password, encryption_data)?,
                    &encryption_data.blake3_salt,
                    &mut derived_key,
                )
                .map_err(|e| anyhow!("Failed to do Argon2: {:?}", e))?;
            derived_key
        };
        blake3::Hasher::new_keyed(&derived_key)
    })
}

#[cfg(test)]
mod tests {
    use argon2::{password_hash::Salt, Argon2};

    use crate::init_encryption_data::init_encryption_data;

    use super::get_hasher;

    #[test]
    fn test_argon2() {
        let mut output = [Default::default(); 4];
        Argon2::default()
            .hash_password_into(
                b"hello",
                &[Default::default(); Salt::RECOMMENDED_LENGTH],
                &mut output,
            )
            .unwrap();
        println!("{output:x?}");
    }

    #[test]
    fn works() {
        let mut hasher = {
            let password = b"password";
            get_hasher(password, &init_encryption_data(password).unwrap())
        }
        .unwrap();
        let hash = hasher.update(b"banned_books").finalize();
        println!("Hash: {hash}");
    }
}
