use aead::Key;
use aes_gcm::Aes256Gcm;
use rand::{thread_rng, RngCore};

pub fn create_immutable_key() -> Key<Aes256Gcm> {
    let mut immutable_key = Key::<Aes256Gcm>::default();
    thread_rng().fill_bytes(immutable_key.as_mut_slice());
    immutable_key
}

#[cfg(test)]
pub mod tests {
    use crate::create_immutable_key::create_immutable_key;

    #[test]
    fn len() {
        assert_eq!(create_immutable_key().len(), 32);
    }
}
