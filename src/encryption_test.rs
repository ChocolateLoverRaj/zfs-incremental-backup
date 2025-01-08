use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
    Key, // Or `Aes128Gcm`
    Nonce,
};

#[cfg(test)]
mod test {
    use std::fmt::Display;

    use aead::{
        generic_array::sequence::GenericSequence,
        stream::{Encryptor, EncryptorBE32},
        Nonce,
    };
    use aes_gcm::aead::AeadMutInPlace;

    use super::*;

    #[test]
    pub fn encrypt_and_decrypt() {
        // The encryption key can be generated randomly:
        let key = Aes256Gcm::generate_key(OsRng);

        let cipher = Aes256Gcm::new(&key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
        let ciphertext = cipher
            .encrypt(&nonce, b"plaintext message".as_ref())
            .unwrap();
        let plaintext = cipher.decrypt(&nonce, ciphertext.as_ref()).unwrap();
        assert_eq!(&plaintext, b"plaintext message");
    }

    #[test]
    pub fn encrypt_and_decrypt_in_place() {
        // The encryption key can be generated randomly:
        let key = Aes256Gcm::generate_key(OsRng);

        let mut cipher = Aes256Gcm::new(&key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
        let mut data = b"plaintext message".to_owned();
        println!(
            "Data to encrypt: {:?} {:x?}",
            String::from_utf8(data.to_vec()),
            data
        );
        let tag = cipher
            .encrypt_in_place_detached(&nonce, &[], &mut data)
            .unwrap();
        println!(
            "Encrypted data: {:?} {:x?}",
            String::from_utf8(data.to_vec()),
            data
        );
        cipher
            .decrypt_in_place_detached(&nonce, &[], &mut data, &tag)
            .unwrap();
        println!(
            "Decrypted data: {:?} {:x?}",
            String::from_utf8(data.to_vec()),
            data
        );
        assert_eq!(&data, b"plaintext message");
    }

    #[test]
    pub fn encrypt_and_decrypt_stream() {
        // The encryption key can be generated randomly:
        let key = Aes256Gcm::generate_key(OsRng);

        let cipher = Aes256Gcm::new(&key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
        let data = b"plaintext message".to_owned();

        fn print_bytes(message: impl Display, bytes: &[u8]) {
            println!(
                "{}: {:?} {:x?}",
                message,
                String::from_utf8(bytes.to_vec()),
                bytes
            );
        }

        let stream = EncryptorBE32::from_aead(cipher, &);
    }
}
