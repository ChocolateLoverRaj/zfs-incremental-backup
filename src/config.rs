// IMPORTANT: Changing these will make previously created buckets not usable by this program
pub const HOT_DATA_OBJECT_KEY: &str = "hot_data";
pub const SNAPSHOTS_PREFIX: &str = "snapshots";
/// Every encryption chunk gets a 16B auth tag, so it's good to have a big chunk size to reduce overhead of auth tags.
/// However, the entire chunk must be in memory, so it shouldn't be too big.
/// It's good for this to be a multiple of 64
/// I set this to 10MB
pub const ENCRYPTION_CHUNK_SIZE: usize = 10_000_000;
// pub const ENCRYPTION_CHUNK_SIZE: usize = 50;

/// The max *upload* size for S3
/// This is currently set to 5GB, in bytes, which is the AWS limit.
pub const MAX_OBJECT_SIZE: u64 = 5 * 1000 * 1000 * 1000;
// For testing with small files, set this to lower
// pub const MAX_OBJECT_SIZE: u64 = 50;

#[cfg(test)]
mod tests {
    use super::ENCRYPTION_CHUNK_SIZE;

    #[test]
    fn encryption_chunk_size_is_multiple_of_64() {
        assert_eq!(ENCRYPTION_CHUNK_SIZE % 64, 0);
    }
}
