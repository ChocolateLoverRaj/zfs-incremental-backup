use serde::{Deserialize, Serialize};

use crate::encryption_password::EncryptionPassword;

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// You can change the encryption password later, but you can't change from Some to None or None to Some.
    /// You can set the encryption password to an empty string to be able to set a password later.
    pub password: EncryptionPassword,
    /// If set to true, the encryption password will be needed to view snapshot names, and object keys will use a secure hash of the password names instead of the actual names.
    pub encrypt_snapshot_names: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupConfig {
    pub encryption: Option<EncryptionConfig>,
    /// We use the name and not the id cuz `zfs snapshot` needs the name and not the id
    /// Example: `zfs-user-files/long-term`
    pub zfs_dataset_name: String,
    // /// The upload speed in Mbps (megabits per second)
    // /// Used for calculating the most cost-effective way of uploading data
    // /// You can use a speed test to get the upload speed
    // pub upload_speed_mbps: f64,
    /// If set to `true`, then an S3 object with 0 bytes size will be created for empty backups. Useful for seeing folders in S3.
    pub create_empty_objects: bool,
}
