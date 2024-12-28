use serde::{Deserialize, Serialize};

use crate::encryption_password::EncryptionPassword;

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupConfig {
    /// You can change the encryption password later, but you can't change from Some to None or None to Some.
    /// You can set the encryption password to an empty string to be able to set a password later.
    pub encryption_password: Option<EncryptionPassword>,
    /// We use the name and not the id cuz `zfs snapshot` needs the name and not the id
    /// Example: `zfs-user-files/long-term`
    pub zfs_dataset_name: String,
}
