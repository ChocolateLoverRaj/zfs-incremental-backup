use std::io;

use anyhow::anyhow;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    primitives::{ByteStream, SdkBody},
    types::StorageClass,
};
use bytes::Bytes;
use chrono::Utc;
use futures::{stream, FutureExt, StreamExt, TryFutureExt, TryStreamExt};
use humansize::{format_size, DECIMAL};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::{
    backup_config::BackupConfig,
    backup_data::{BackupData, BackupState},
    config::SNAPSHOTS_PREFIX,
    diff_or_first::{diff_or_first, FileType},
    retry_steps::{RetryStepOutput, StepDoer},
    zfs_mount_get::zfs_mount_get,
    zfs_take_snapshot::zfs_take_snapshot,
};

pub struct BackupSteps {
    pub take_snapshot: bool,
    pub snapshot_name: Option<String>,
    pub config: BackupConfig,
}

impl StepDoer<BackupData, (), anyhow::Error, anyhow::Error> for BackupSteps {
    async fn do_step<'a>(
        &'a mut self,
        mut backup_data: BackupData,
        state_saver: &mut impl crate::retry_steps::StateSaver<BackupData, anyhow::Error>,
    ) -> Result<crate::retry_steps::RetryStepOutput<BackupData, ()>, anyhow::Error> {
        match backup_data.backup_state.as_mut() {
            None => {
                let snapshot_name = if self.take_snapshot {
                    // Don't backup more than once a second please. It won't work.
                    let snapshot_name = self
                        .snapshot_name
                        .clone()
                        .unwrap_or(format!("backup-{}", Utc::now().format("%Y-%m-%d_%H-%M-%S")));
                    println!("Snapshot name: {snapshot_name:?}");
                    zfs_take_snapshot(&self.config.zfs_dataset_name, &snapshot_name).await?;
                    println!("Took snapshot");
                    snapshot_name
                } else {
                    self.snapshot_name.clone().ok_or(anyhow!(
                        "Must specify a snapshot name, or use --take-snapshot"
                    ))?
                };
                // TODO: Handle crashing between taking snapshot and saving state. If we don't, then there could be unused snapshots
                // Fail if we couldn't save state because we could end up wasting time
                state_saver.save_state(&backup_data).await?;
                backup_data.backup_state = Some(BackupState {
                    snapshot_name: snapshot_name.clone(),
                    diff: None,
                });
                Ok(RetryStepOutput::NotFinished(backup_data))
            }
            Some(backup_state) => {
                match backup_state.diff.as_mut() {
                    None => {
                        // TODO: When scanning files for the first snapshot, we could continue where we left off if we fail
                        println!("Diffing...");
                        let diff = stream::iter(
                            diff_or_first(
                                &self.config.zfs_dataset_name,
                                backup_data.last_saved_snapshot_name.as_deref(),
                                &backup_state.snapshot_name,
                            )
                            .await?
                            .into_iter(),
                        )
                        .flat_map_unordered(None, |diff_entry| {
                            let path = diff_entry.path.clone();
                            let file_type = diff_entry.file_type.clone();
                            diff_entry
                                .try_map_async(move |option| {
                                    {
                                        let value = path.clone();
                                        let file_type = file_type.clone();
                                        async move {
                                            Ok::<_, io::Error>(match option {
                                                Some(len) => Some(len),
                                                None => match file_type {
                                                    FileType::RegularFile => {
                                                        // TODO: Save metadata progress so retries don't need to get all the metadata again
                                                        Some(
                                                            tokio::fs::metadata(value.clone())
                                                                .await?
                                                                .len(),
                                                        )
                                                    }
                                                    FileType::Directory => None,
                                                },
                                            })
                                        }
                                    }
                                    .boxed()
                                })
                                .into_stream()
                                .boxed()
                        })
                        .try_collect::<Vec<_>>()
                        .await?;
                        println!("Diff: {diff:#?}");
                        backup_state.diff = Some(diff);
                        // We fail on save error because we don't want to keep re-diffing everything, which could involve reading thousands of files
                        state_saver.save_state(&backup_data).await?;
                        Ok(RetryStepOutput::NotFinished(backup_data))
                    }
                    Some(diff) => {
                        let snapshot_upload_size = diff.iter().try_fold(0, |sum, diff_entry| {
                            let postcard_len = postcard::to_allocvec(diff_entry)?.len() as u64;
                            anyhow::Ok(
                                sum
                                        // Length of the postcard
                                        + varint_simd::encode(postcard_len).1 as u64
                                        // Postcard also contain length of content
                                        + postcard_len
                                        // The content (for create / modify)
                                        + diff_entry.diff_type.content_data().copied().flatten().unwrap_or(0),
                            )
                        })?;
                        println!(
                            "Snapshot upload size: {}",
                            format_size(snapshot_upload_size, DECIMAL)
                        );

                        // TODO: We could save space by not including the full path
                        // TODO: Maybe upload smaller files or use multipart upload in case 5GB uploads fail
                        let sdk_config =
                            aws_config::defaults(BehaviorVersion::latest()).load().await;
                        let s3_client = aws_sdk_s3::Client::new(&sdk_config);
                        println!("Uploading test");
                        s3_client
                            .put_object()
                            // TODO: Deep Archive
                            .storage_class(StorageClass::Standard)
                            .bucket(&backup_data.s3_bucket)
                            .key(format!(
                                "{}/{}/0",
                                SNAPSHOTS_PREFIX, &backup_state.snapshot_name
                            ))
                            .body({
                                let mount_point = zfs_mount_get(&self.config.zfs_dataset_name)
                                    .await?
                                    .ok_or(anyhow!("Not mounted"))?;
                                let stream =
                                    stream::iter(diff.to_vec()).flat_map(move |diff_entry| {
                                        // FIXME: Don't panic here
                                        let postcard_bytes =
                                            postcard::to_allocvec(&diff_entry).unwrap();
                                        let postcard_size_bytes = {
                                            let mut postcard_size_bytes = vec![u8::default(); 10];
                                            let postcard_size_bytes_len =
                                                varint_simd::encode_to_slice(
                                                    postcard_bytes.len() as u64,
                                                    postcard_size_bytes.as_mut_slice(),
                                                );
                                            postcard_size_bytes
                                                .truncate(postcard_size_bytes_len as usize);
                                            postcard_size_bytes
                                        };
                                        let s = stream::iter([
                                            Ok(Bytes::from(postcard_size_bytes)),
                                            Ok(postcard_bytes.into()),
                                        ]);
                                        match diff_entry.diff_type.content_data().copied().flatten()
                                        {
                                            Some(_file_len) => s
                                                .chain(
                                                    File::open(mount_point.join(diff_entry.path))
                                                        .map(|result| {
                                                            result
                                                                .map(|file| ReaderStream::new(file))
                                                        })
                                                        .try_flatten_stream(),
                                                )
                                                .boxed(),
                                            None => s.boxed(),
                                        }
                                    });
                                let body = reqwest::Body::wrap_stream(stream);
                                let byte_stream = ByteStream::new(SdkBody::from_body_1_x(body));
                                byte_stream
                            })
                            .content_length(snapshot_upload_size as i64)
                            .send()
                            .await?;
                        println!("Uploaded test");
                        Ok(RetryStepOutput::Finished(()))
                    }
                }
            }
        }
    }
}
