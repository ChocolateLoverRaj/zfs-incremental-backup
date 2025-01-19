use std::{
    borrow::Cow,
    io::{ErrorKind, Write},
    path::PathBuf,
};

use anyhow::{anyhow, Context};
use aws_config::BehaviorVersion;
use aws_smithy_types_convert::stream::PaginationStreamImplStream;
use clap::Parser;
use futures::{future::try_join, StreamExt, TryFutureExt, TryStreamExt};
use promptuity::{
    prompts::{Input, Password, Select, SelectOption},
    themes::MinimalTheme,
    Promptuity, Term,
};
use shallowclone::ShallowClone;
use tokio::{
    fs::{read_dir, OpenOptions},
    io::AsyncWriteExt,
};
use tokio_stream::wrappers::ReadDirStream;

use crate::{
    backup_config::{BackupConfig, EncryptionConfig},
    backup_data::BackupData,
    encryption_password::EncryptionPassword,
    remote_hot_data::{download_hot_data_encrypted, RemoteHotData, RemoteHotDataEncrypted},
    zfs_list_snapshots::zfs_list_snapshots,
    zfs_mount_get::zfs_mount_get,
};

#[derive(Parser)]
pub struct RecoverConfigCommand {
    /// Path to a JSON file with config
    #[arg(short, long)]
    config_path: PathBuf,
    /// Path to the backup data JSON file
    #[arg(short, long)]
    data_path: PathBuf,
    /// Overwrite the data file
    #[arg(short, long)]
    force: bool,
    #[arg(short, long)]
    s3_bucket: Option<String>,
    #[arg(short, long)]
    zfs_dataset_name: String,
    #[arg(long)]
    create_empty_objects: bool,
}

pub async fn recover_config_command(
    RecoverConfigCommand {
        config_path,
        data_path,
        force,
        s3_bucket,
        zfs_dataset_name,
        create_empty_objects,
    }: RecoverConfigCommand,
) -> anyhow::Result<()> {
    if !zfs_list_snapshots(&zfs_dataset_name).await?.is_empty() {
        Err(anyhow!("Dataset must not have any snapshots"))?;
    };
    let mount_point = zfs_mount_get(&zfs_dataset_name)
        .await?
        .ok_or(anyhow!("dataset not mounted"))?;
    if ReadDirStream::new(read_dir(mount_point).await?)
        .count()
        .await
        > 0
    {
        Err(anyhow!("Dataset must not have any files in it"))?;
    };

    let open_options = {
        let mut open_options = OpenOptions::new();
        open_options
            .read(false)
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(!force);
        open_options
    };
    let (mut config_file, mut data_file) = try_join(
        open_options.open(config_path).map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => anyhow::Error::from(e)
                .context("Backup config file already exists. Use -f to overwrite."),
            _ => e.into(),
        }),
        open_options.open(data_path).map_err(|e| match e.kind() {
            ErrorKind::AlreadyExists => anyhow::Error::from(e)
                .context("Backup data file already exists. Use -f to overwrite."),
            _ => e.into(),
        }),
    )
    .await?;

    let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);

    let mut term = Term::default();
    let mut theme = MinimalTheme::default();
    let mut p = Promptuity::new(&mut term, &mut theme);
    p.begin()?;

    let s3_bucket = match s3_bucket {
        Some(bucket) => bucket,
        None => {
            let buckets =
                PaginationStreamImplStream::new(s3_client.list_buckets().into_paginator().send())
                    .try_collect::<Vec<_>>()
                    .await?
                    .into_iter()
                    .flat_map(|output| output.buckets.unwrap_or_default())
                    .map(|bucket| bucket.name.ok_or(anyhow!("No bucket name")))
                    .collect::<Result<Vec<_>, _>>()?;

            let bucket = p.prompt(
                Select::new(
                    "Which S3 bucket?",
                    buckets
                        .into_iter()
                        .map(|bucket| SelectOption::new(bucket.clone(), bucket))
                        .collect(),
                )
                .as_mut(),
            )?;
            bucket
        }
    };
    let backup_config = BackupConfig {
        zfs_dataset_name,
        create_empty_objects,
        encryption: match download_hot_data_encrypted(&s3_client, &s3_bucket).await? {
            RemoteHotData::Encrypted(encrypted) => Some(EncryptionConfig {
                password: {
                    #[derive(Debug, Clone, Copy)]
                    enum PasswordType {
                        Plain,
                        Hex,
                        File,
                    }
                    let password_type = p.prompt(Select::new("The backup is encrypted. How do you want to configure the encryption password?", [
                        SelectOption::new("plain text", Some(PasswordType::Plain)),
                        SelectOption::new("hex text", Some(PasswordType::Hex)),
                        SelectOption::new("file containing password", Some(PasswordType::File))
                    ].to_vec()).as_mut())?.ok_or(anyhow!("No password type"))?;

                    // let password_that_works =
                    //     |get_password: &mut dyn FnMut() -> EncryptionPassword| async move {
                    //         loop {
                    //             let password = get_password();
                    //             password.get_bytes().await?;
                    //         }
                    //         anyhow::Ok(())
                    //     };

                    async fn get_password_that_works<
                        F: FnMut(&mut Promptuity<'_, W>) -> anyhow::Result<EncryptionPassword>,
                        W: Write,
                    >(
                        p: &mut Promptuity<'_, W>,
                        encrypted: RemoteHotDataEncrypted<'_>,
                        mut prompt_password: F,
                    ) -> anyhow::Result<EncryptionPassword> {
                        Ok(loop {
                            let password = prompt_password(p)?;
                            match password.get_bytes().await {
                                Ok(encryption_password) => {
                                    match encrypted.shallow_clone().decrypt(&encryption_password) {
                                        Ok(_) => {
                                            break password;
                                        }
                                        Err(e) => {
                                            p.error(e.context("Error decrypting backup. Probably cuz the password is incorrect. If using a file, check whitespace."))?
                                        }
                                    }
                                }
                                Err(err) => p.error(
                                    err.context("Error parsing / reading encryption password"),
                                )?,
                            };
                        })
                    }

                    match password_type {
                        PasswordType::Plain => {
                            get_password_that_works(&mut p, encrypted, |p| {
                                Ok(EncryptionPassword::Plain(p.prompt(
                                    Password::new("Type the encryption password").as_mut(),
                                )?))
                            })
                            .await?
                        }
                        PasswordType::Hex => {
                            get_password_that_works(&mut p, encrypted, |p| {
                                Ok(EncryptionPassword::Plain(p.prompt(
                                    Password::new("Type the encryption password hex").as_mut(),
                                )?))
                            })
                            .await?
                        }
                        PasswordType::File => {
                            get_password_that_works(&mut p, encrypted, |p| {
                                Ok(EncryptionPassword::File(
                                    p.prompt(Input::new("Path to the encryption file").as_mut())?
                                        .into(),
                                ))
                            })
                            .await?
                        }
                    }
                },
            }),
            RemoteHotData::NotEncrypted(_data) => None,
        },
    };
    let backup_data = BackupData {
        s3_bucket: Cow::Owned(s3_bucket),
        last_saved_snapshot_name: None,
        backup_step: None,
    };

    p.finish()?;

    try_join(
        config_file.write_all(serde_json::to_string_pretty(&backup_config)?.as_bytes()),
        data_file.write_all(serde_json::to_string_pretty(&backup_data)?.as_bytes()),
    )
    .await
    .context("Failed to write files")?;

    println!("Saved config and data files");

    Ok(())
}
