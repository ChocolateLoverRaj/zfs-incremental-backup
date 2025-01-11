#[cfg(test)]
mod test {
    use std::{fmt::Display, time::Instant};

    use aead::{
        stream::{DecryptorBE32, EncryptorBE32},
        Aead, AeadCore, KeyInit, OsRng,
    };
    use aes_gcm::{aead::AeadMutInPlace, Aes256Gcm};
    use chacha20::{
        cipher::{KeyIvInit, StreamCipher, StreamCipherSeek},
        ChaCha20,
    };

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
    pub fn encrypt_and_decrypt_block_stream() {
        // The encryption key can be generated randomly:
        let key = Aes256Gcm::generate_key(OsRng);

        let cipher = Aes256Gcm::new(&key);
        let data = b"plaintext message".to_owned();

        fn print_bytes(message: impl Display, bytes: &[u8]) {
            println!(
                "{}: {:?} {:x?} {}",
                message,
                String::from_utf8(bytes.to_vec()),
                bytes,
                bytes.len()
            );
        }
        print_bytes("Unencrypted", &data);

        let nonce = [0u8; 7];
        let mut stream = EncryptorBE32::from_aead(cipher.clone(), nonce.as_ref().into());
        let mut chunk_0 = data[..16].to_vec();
        print_bytes("Chunk 0 unencrypted", &chunk_0);
        stream.encrypt_next_in_place(&[], &mut chunk_0).unwrap();
        print_bytes("Chunk 0 encrypted", &chunk_0);
        let mut chunk_1 = data[16..].to_vec();
        print_bytes("Chunk 1 unencrypted", &chunk_1);
        stream.encrypt_last_in_place(&[], &mut chunk_1).unwrap();
        print_bytes("Chunk 1 encrypted", &chunk_1);

        // *chunk_1.last_mut().unwrap() += 1;

        let mut d = DecryptorBE32::from_aead(cipher, nonce.as_ref().into());
        d.decrypt_next_in_place(&[], &mut chunk_0).unwrap();
        print_bytes("Chunk 0 decrypted", &chunk_0);
        d.decrypt_last_in_place(&[], &mut chunk_1).unwrap();
        print_bytes("Chunk 1 decrypted", &chunk_1);
    }

    #[test]
    pub fn encrypt_and_decrypt_block_stream_2() {
        // The encryption key can be generated randomly:
        let key = Aes256Gcm::generate_key(OsRng);

        let cipher = Aes256Gcm::new(&key);
        let plaintext = b"plaintext message";

        fn print_bytes(message: impl Display, bytes: &[u8]) {
            println!(
                "{}: {:?} {:x?} {}",
                message,
                String::from_utf8(bytes.to_vec()),
                bytes,
                bytes.len()
            );
        }

        print_bytes("plaintext", plaintext);
        let nonce = [0u8; 7];
        const CHUNK_SIZE: usize = 1;
        let ciphertext = {
            let mut ciphertext = Vec::default();
            let mut encryptor = EncryptorBE32::from_aead(cipher.clone(), nonce.as_ref().into());
            let total_chunks = plaintext.len().div_ceil(CHUNK_SIZE);
            for i in 0..total_chunks - 1 {
                ciphertext.append(
                    &mut encryptor
                        .encrypt_next(&plaintext[i * CHUNK_SIZE..(i + 1) * CHUNK_SIZE])
                        .unwrap(),
                );
            }
            ciphertext.append(
                &mut encryptor
                    .encrypt_last(&plaintext[(total_chunks - 1) * CHUNK_SIZE..])
                    .unwrap(),
            );
            ciphertext
        };
        print_bytes("ciphertext", &ciphertext);

        let decrypted = {
            let mut decrypted = Vec::default();
            let mut decryptor = DecryptorBE32::from_aead(cipher, nonce.as_ref().into());
            let total_chunks = ciphertext.len().div_ceil(CHUNK_SIZE + 16);
            for i in 0..total_chunks - 1 {
                decrypted.append(
                    &mut decryptor
                        .decrypt_next(
                            &ciphertext[i * (CHUNK_SIZE + 16)..(i + 1) * (CHUNK_SIZE + 16)],
                        )
                        .unwrap(),
                );
            }
            decrypted.append(
                &mut decryptor
                    .decrypt_last(&ciphertext[(total_chunks - 1) * (CHUNK_SIZE + 16)..])
                    .unwrap(),
            );
            decrypted
        };
        print_bytes("plaintext", &decrypted);
    }

    #[test]
    pub fn encrypt_and_decrypt_stream() {
        let key = [0x42; 32];
        let nonce = [0x24; 12];
        let plaintext = b"Rust is fun";
        println!("{} - plaintext", hex::encode(plaintext));
        const CHUNK_SIZE: usize = 10;
        let encrypted_data = {
            let mut encrypted_data = Vec::default();
            for (chunk_index, chunk) in plaintext.chunks(CHUNK_SIZE).enumerate() {
                // Variable is created in each iteration to simulate restarting the program for each chunk
                let mut cipher = ChaCha20::new(&key.into(), &nonce.into());
                let mut buffer = chunk.to_owned();
                cipher.try_seek(chunk_index * CHUNK_SIZE).unwrap();
                cipher.try_apply_keystream(&mut buffer).unwrap();
                println!("{} - encrypted chunk {}", hex::encode(&buffer), {
                    chunk_index
                });
                encrypted_data.extend(buffer);
            }
            encrypted_data
        };
        println!("{} - encrypted", hex::encode(&encrypted_data));
        let decrypted_data = {
            let mut decrypted_data = Vec::default();
            for (chunk_index, chunk) in encrypted_data.chunks(CHUNK_SIZE).enumerate() {
                // Variable is created in each iteration to simulate restarting the program for each chunk
                let mut cipher = ChaCha20::new(&key.into(), &nonce.into());
                let mut buffer = chunk.to_owned();
                cipher.try_seek(chunk_index * CHUNK_SIZE).unwrap();
                cipher.try_apply_keystream(&mut buffer).unwrap();
                println!("{} - decrypted chunk {}", hex::encode(&buffer), chunk_index);
                decrypted_data.extend(buffer);
            }
            decrypted_data
        };
        println!("{} - decrypted", hex::encode(&decrypted_data));
        assert_ne!(plaintext, encrypted_data.as_slice());
        assert_eq!(plaintext, decrypted_data.as_slice());
    }

    #[test]
    fn big_seek() {
        let key = [0x42; 32];
        let nonce = [0x24; 12];
        let mut cipher = ChaCha20::new(&key.into(), &nonce.into());
        // const SEEK_AMOUNT: usize = 536_870_912_000; // Doesn't work cuz >32bit number
        const SEEK_AMOUNT: usize = 4294967295;
        // const SEEK_AMOUNT: usize = 1;
        // cipher.seek(SEEK_AMOUNT);
        let mut buffer = [Default::default(); 64 * 1000];
        let before = Instant::now();
        for _ in 0..SEEK_AMOUNT.div_ceil(buffer.len()) {
            cipher.apply_keystream(&mut buffer);
        }
        let after = Instant::now();
        println!("Done in {:?}", after - before);
    }
}
