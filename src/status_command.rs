use std::{borrow::Cow, cell::RefCell, path::PathBuf, sync::Arc};

use aws_config::BehaviorVersion;
use clap::Parser;
use futures::{stream, StreamExt, TryStreamExt};
use humansize::{format_size, DECIMAL};
use shallowclone::ShallowClone;
use tabled::{Table, Tabled};

use crate::{
    encryption_password::EncryptionPassword, get_config::get_config, get_data::get_data,
    get_snapshot_len::get_snapshot_len, remote_hot_data::download_hot_data,
};

#[derive(Parser)]
pub struct StatusCommand {
    /// Path to a JSON file with config
    #[arg(short, long)]
    config_path: PathBuf,
    /// Path to the backup data JSON file
    #[arg(short, long)]
    data_path: PathBuf,
}

pub async fn status_command(
    StatusCommand {
        config_path,
        data_path,
    }: StatusCommand,
) -> anyhow::Result<()> {
    let config = Arc::new(get_config(&config_path).await?);
    let data = Arc::new(get_data(&data_path).await?);

    println!(
        "You are backing up the zfs dataset {:?}",
        config.zfs_dataset_name
    );
    match &config.encryption {
        None => {
            println!("Files stored on the cloud are not encrypted from the cloud service");
        }
        Some(encryption_config) => {
            println!("Files stored on the cloud are encrypted. To restore, you will need your encryption password in addition to having access to the cloud resource.");
            match &encryption_config.password {
                EncryptionPassword::File(file) => {
                    println!("You are storing your encryption password in the file {:?}. Make sure you will have this file available when you restore. Keep this file a secret.", file);
                }
                EncryptionPassword::Hex(_) | EncryptionPassword::Plain(_) => {
                    println!("You are storing your encryption password in the config file ({:?}). Make sure you have your config file or password available when you restore. Keep your config file a secret.", config_path);
                }
            }
            if encryption_config.encrypt_snapshot_names {
                println!("You are also encrypting the snapshot names themselves.");
            } else {
                println!("The files are encrypted, but the snapshot names are not");
            }
        }
    }

    println!(
        "The backups are saved to the AWS S3 bucket {:?} in the AWS S3 region {:?}",
        data.s3_bucket, data.s3_region
    );
    println!(
        "The last backed-up snapshot is {:?}",
        data.last_saved_snapshot_name
    );
    if data.backup_step.is_some() {
        println!("There is a backup in progress. It may not be running rn.");
    }

    let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
    let s3_client = Arc::new(aws_sdk_s3::Client::new(&sdk_config));
    let remote_hot_data = Arc::new(download_hot_data(&config, &s3_client, &data.s3_bucket).await?);

    #[derive(Tabled)]
    struct TableRow<'a> {
        name: Cow<'a, str>,
        size: Cow<'a, str>,
        cumulative_size: Cow<'a, str>,
    }

    let rows = stream::iter(remote_hot_data.snapshots.iter())
        .then({
            let cumulative_size = Arc::new(RefCell::new(0));
            let remote_hot_data = remote_hot_data.clone();
            move |snapshot| {
                let config = config.clone();
                let remote_hot_data = remote_hot_data.clone();
                let s3_client = s3_client.clone();
                let data = data.clone();
                let cumulative_size = cumulative_size.clone();
                async move {
                    anyhow::Ok({
                        let size = get_snapshot_len(
                            &s3_client,
                            &config,
                            data.shallow_clone(),
                            remote_hot_data.shallow_clone(),
                            snapshot.as_ref(),
                        )
                        .await?;
                        *cumulative_size.borrow_mut() += size;
                        TableRow {
                            name: snapshot.shallow_clone(),
                            size: Cow::Owned(format_size(size, DECIMAL)),
                            cumulative_size: Cow::Owned(format_size(
                                *cumulative_size.borrow_mut(),
                                DECIMAL,
                            )),
                        }
                    })
                }
            }
        })
        .try_collect::<Vec<_>>()
        .await?;
    println!("Snapshots uploaded:");
    println!("{}", Table::new(rows).to_string());
    println!("The table shows the size on the cloud, but if you restore it then the size on disk may be different, depending on ZFS settings and encryption settings.");

    Ok(())
}
