use std::borrow::Cow;

use anyhow::anyhow;

use crate::{
    backup_config::BackupConfig, get_hasher::get_hasher, remote_hot_data::RemoteHotDataInMemory,
};

pub async fn get_encrypted_snapshot_name<'a>(
    config: &'a BackupConfig,
    remote_hot_data: RemoteHotDataInMemory<'a>,
    snapshot_name: &'a str,
) -> anyhow::Result<Cow<'a, str>> {
    let snapshot_name = {
        match &config.encryption {
            Some(encryption_config) => {
                // Snapshot names are currently always encrypted
                if true {
                    let encryption_data = remote_hot_data
                        .encryption
                        .as_deref()
                        .ok_or(anyhow!("No encryption data"))?;
                    Cow::Owned(
                        get_hasher(
                            &encryption_config.password.get_bytes().await?,
                            encryption_data,
                        )?
                        .update(snapshot_name.as_bytes())
                        .finalize()
                        .to_string(),
                    )
                } else {
                    Cow::Borrowed(snapshot_name)
                }
            }
            None => Cow::Borrowed(snapshot_name),
        }
    };
    Ok(snapshot_name)
}
