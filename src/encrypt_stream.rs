use std::borrow::Borrow;

use aead::{stream::EncryptorBE32, KeyInit};
use aes_gcm::Aes256Gcm;
use anyhow::anyhow;
use bytes::Bytes;
use futures::{Stream, StreamExt};

use crate::{decrypt_immutable_key::decrypt_immutable_key, remote_hot_data::EncryptionData};

pub trait EncryptStream<E> {
    fn encrypt(
        self,
        password: impl Borrow<[u8]>,
        encryption_data: impl Borrow<EncryptionData>,
        nonce: [u8; 7],
        total_chunks: usize,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<Bytes>>>;
}

impl<S, E: Into<anyhow::Error>> EncryptStream<E> for S
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    fn encrypt(
        self,
        password: impl Borrow<[u8]>,
        encryption_data: impl Borrow<EncryptionData>,
        nonce: [u8; 7],
        total_chunks: usize,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<Bytes>>> {
        Ok({
            let cipher = Aes256Gcm::new_from_slice(&decrypt_immutable_key(
                password.borrow(),
                encryption_data.borrow(),
            )?)?;
            let mut encryptor = Some(EncryptorBE32::from_aead(cipher, nonce.as_ref().into()));
            let mut chunks_encrypted = 0;
            self.map(move |chunk| {
                Ok({
                    let payload = &chunk.map_err(|e| e.into())?[..];
                    let encrypted_chunk = if chunks_encrypted + 1 < total_chunks {
                        encryptor.as_mut().unwrap().encrypt_next(payload)
                    } else {
                        encryptor.take().unwrap().encrypt_last(payload)
                    }
                    .map_err(|e| anyhow!("Failed to encrypt chunk: {:?}", e))?;
                    chunks_encrypted += 1;
                    encrypted_chunk.into()
                })
            })
        })
    }
}
