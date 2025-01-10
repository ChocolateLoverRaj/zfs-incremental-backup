use std::{borrow::Cow, path::PathBuf};

use anyhow::anyhow;
use aws_config::BehaviorVersion;
use clap::Parser;
use promptuity::{prompts::Password, themes::MinimalTheme, Promptuity, Term};

use crate::{
    check_key::decrypt_immutable_key,
    derive_key::{encrypt_immutable_key, generate_salt_and_derive_key},
    get_config::get_config,
    get_data::get_data,
    remote_hot_data::{download_hot_data, upload_hot_data, EncryptionData},
};

#[derive(Parser)]
pub struct ChangePasswordCommand {
    /// Path to a JSON file with config
    #[arg(short, long)]
    config_path: PathBuf,
    /// Path to the backup data JSON file
    #[arg(short, long)]
    data_path: PathBuf,
}

pub async fn change_password_command(
    ChangePasswordCommand {
        config_path,
        data_path,
    }: ChangePasswordCommand,
) -> anyhow::Result<()> {
    let backup_config = get_config(config_path).await?;
    let backup_data = get_data(&data_path).await?;
    match backup_config.encryption {
        Some(encryption_config) => {
            let encryption_password = encryption_config.password.get_bytes().await?;
            let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
            let s3_client = aws_sdk_s3::Client::new(&sdk_config);
            let mut remote_hot_data = download_hot_data(&s3_client, &backup_data.s3_bucket).await?;
            let decrypted_immutable_key =
                        decrypt_immutable_key(&encryption_password, remote_hot_data.encryption.as_deref().ok_or(anyhow!("The local config specifies an encryption password, but the remote data is not encrypted."))?)?;

            let mut term = Term::default();
            let mut theme = MinimalTheme::default();
            let mut p = Promptuity::new(&mut term, &mut theme);

            let new_password = {
                p.begin()?;
                let password = loop {
                    let password =
                        p.prompt(Password::new("Set a new encryption password").with_hint(
                            "If you need an alternate method other than stdin, open an issue",
                        ))?;
                    let password_repeated =
                        p.prompt(Password::new("Re-enter the new encryption password").as_mut())?;
                    if password_repeated == password {
                        break password;
                    }
                    p.error("Repeated password was not the same")?;
                };
                p.finish()?;
                password
            };
            let (new_salt, new_derived_key) =
                generate_salt_and_derive_key(new_password.as_bytes())?;
            let encrypted_immutable_key =
                encrypt_immutable_key(&new_derived_key, &decrypted_immutable_key)?;
            remote_hot_data.encryption = Some(Cow::Owned(EncryptionData {
                password_derived_key_salt: new_salt,
                encrypted_immutable_key,
            }));
            upload_hot_data(&s3_client, &backup_data.s3_bucket, &remote_hot_data).await?;
            println!("Changed encryption password. Make sure to update your config to use the new password because the previous password will not work. You can use `check-password` to check it.");
        }
        None => {
            Err(anyhow!("Not encrypted! There is no way to encrypt the unencrypted backups. You will have to create a new encrypted backup and delete the old one."))?;
        }
    }
    Ok(())
}
