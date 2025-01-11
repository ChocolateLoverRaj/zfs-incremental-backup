use std::{borrow::Cow, io, ops::Deref, rc::Rc, time::Duration};

use anyhow::anyhow;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{error::SdkError, primitives::ByteStream, types::StorageClass};
use bytes::Bytes;
use chrono::Utc;
use futures::{stream, FutureExt, StreamExt, TryStreamExt};
use humansize::{format_size, DECIMAL};
use shallowclone::ShallowClone;
use spinners::{Spinner, Spinners};

use crate::{
    backup_config::BackupConfig,
    backup_data::{BackupData, BackupStep, BackupStepDiff},
    chunks_stream::{ChunksStreamExt, ChunksStreamOfStreams},
    config::SNAPSHOTS_PREFIX,
    diff_or_first::{diff_or_first, FileType},
    remote_hot_data::{download_hot_data, upload_hot_data, RemoteHotDataDecrypted},
    retry_steps_2::{RetryStepNotFinished2, RetryStepOutput2, StepDoer2},
    sleep_with_spinner::sleep_with_spinner,
    snapshot_upload_stream_2::snapshot_upload_stream,
    zfs_mount_get::zfs_snapshot_mount_get,
    zfs_take_snapshot::zfs_take_snapshot,
};

pub struct BackupSteps<'a> {
    pub config: BackupConfig,
    pub backup_data: Rc<BackupData<'a>>,
    pub remote_hot_data: Option<RemoteHotDataDecrypted<'a>>,
}

impl<'a> BackupSteps<'a> {
    pub async fn start<'b>(
        &mut self,
        take_snapshot: bool,
        snapshot_name: Option<Cow<'b, str>>,
        allow_empty: bool,
        s3_client: &aws_sdk_s3::Client,
        // hot_data: RemoteHotDataDecrypted<'b>,
    ) -> anyhow::Result<RetryStepNotFinished2<M, BackupStep<'b>>> {
        let snapshot_name = if take_snapshot {
            // Don't backup more than once a second please. It won't work.
            let snapshot_name = snapshot_name.unwrap_or(Cow::Owned(format!(
                "backup-{}",
                Utc::now().format("%Y-%m-%d_%H-%M-%S")
            )));
            println!("Snapshot name: {snapshot_name:?}");
            zfs_take_snapshot(&self.config.zfs_dataset_name, &snapshot_name).await?;
            println!("Took snapshot");
            snapshot_name
        } else {
            snapshot_name.ok_or(anyhow!(
                "Must specify a snapshot name, or use --take-snapshot"
            ))?
        };
        let hot_data = self.take_remote_hot_data(s3_client).await?;
        match hot_data
            .snapshots
            .iter()
            .map(|saved_snapshot_name| saved_snapshot_name.deref())
            .find(|saved_snapshot_name| *saved_snapshot_name == snapshot_name.deref())
        {
            None => Ok(()),
            Some(name) => Err(anyhow!("Snapshot with name {:?} already saved", name)),
        }?;
        self.remote_hot_data = Some(hot_data);
        // TODO: Handle crashing between taking snapshot and saving state. If we don't, then there could be unused snapshots
        Ok(RetryStepNotFinished2 {
            memory_data: None,
            persistent_data: BackupStep::Diff(BackupStepDiff {
                snapshot_name,
                allow_empty,
                // hot_data,
            }),
        })
    }

    async fn take_remote_hot_data(
        &mut self,
        s3_client: &aws_sdk_s3::Client,
    ) -> anyhow::Result<RemoteHotDataDecrypted<'a>> {
        Ok({
            let remote_hot_data = match self.remote_hot_data.take() {
                Some(remote_hot_data) => remote_hot_data,
                None => {
                    download_hot_data(&self.config, s3_client, &self.backup_data.s3_bucket).await?
                }
            };
            remote_hot_data
        })
    }
}

type M = Option<ChunksStreamOfStreams<'static, io::Result<Bytes>>>;

impl<'a> StepDoer2<M, BackupStep<'a>, Option<Cow<'a, str>>, anyhow::Error, anyhow::Error>
    for BackupSteps<'a>
{
    async fn do_step(
        &mut self,
        memory_data: M,
        persitent_data: BackupStep<'a>,
    ) -> Result<
        crate::retry_steps_2::RetryStepOutput2<M, BackupStep<'a>, Option<Cow<'a, str>>>,
        anyhow::Error,
    > {
        match persitent_data {
            BackupStep::Diff(backup_step_diff) => {
                // TODO: When scanning files for the first snapshot, we could continue where we left off if we fail
                println!("Diffing...");
                let diff = stream::iter(
                    diff_or_first(
                        &self.config.zfs_dataset_name,
                        self.backup_data.last_saved_snapshot_name.as_deref(),
                        &backup_step_diff.snapshot_name,
                    )
                    .await?
                    .into_iter(),
                )
                .flat_map_unordered(None, |diff_entry| {
                    let path = diff_entry.path.clone();
                    let file_type = diff_entry.file_type;
                    diff_entry
                        .try_map_async(move |option| {
                            {
                                let value = path.clone();
                                async move {
                                    Ok::<_, io::Error>(match option {
                                        Some(len) => Some((&len).into()),
                                        None => match file_type {
                                            FileType::RegularFile => {
                                                // TODO: Save metadata progress so retries don't need to get all the metadata again
                                                Some(
                                                    (&tokio::fs::metadata(value.clone()).await?)
                                                        .into(),
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
                if backup_step_diff.allow_empty || !diff.is_empty() {
                    println!("Diff: {diff:#?}");
                    let step = backup_step_diff.next(diff);
                    Ok(RetryStepOutput2::NotFinished(RetryStepNotFinished2 {
                        memory_data: None,
                        persistent_data: step,
                    }))
                } else {
                    Ok(RetryStepOutput2::Finished(None))
                }
            }
            BackupStep::Upload(mut backup_step_upload) => {
                let snapshot_upload_size = backup_step_upload.diff.iter().try_fold(0, |sum, diff_entry| {
                    let postcard_len = postcard::to_allocvec(diff_entry)?.len() as u64;
                    anyhow::Ok(
                        sum
                                // Length of the postcard
                                + varint_simd::encode(postcard_len).1 as u64
                                // Postcard also contain length of content
                                + postcard_len
                                // The content (for create / modify)
                                + diff_entry.diff_type.content_data().copied().flatten().map_or(0, |file_meta_data| file_meta_data.len),
                    )
                })?;

                // TODO: We could save space by not including the full path
                // TODO: Maybe upload smaller files or use multipart upload in case 5GB uploads fail
                let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
                let s3_client = aws_sdk_s3::Client::new(&sdk_config);

                // 5GB, in bytes
                // let max_object_size: u64 = 5 * 1000 * 1000 * 1000;
                let max_object_size: u64 = 50;

                let total_objects_count = snapshot_upload_size.div_ceil(max_object_size).max(
                    match self.config.create_empty_objects {
                        true => 1,
                        false => 0,
                    },
                );

                if memory_data.is_none() {
                    println!(
                        "Snapshot upload size: {}",
                        format_size(snapshot_upload_size, DECIMAL)
                    );
                    println!(
                        "Snapshots will be uploaded in {} parts",
                        total_objects_count
                    );
                    if backup_step_upload.uploaded_objects > 0 {
                        println!(
                            "{} parts were already uploaded. Starting from there.",
                            backup_step_upload.uploaded_objects
                        )
                    }
                }

                let snapshot_upload_stream = if backup_step_upload.uploaded_objects
                    < total_objects_count
                {
                    let snapshot_upload_stream: ChunksStreamOfStreams<
                        'static,
                        Result<Bytes, io::Error>,
                    > = match memory_data {
                        Some(snapshot_upload_stream) => snapshot_upload_stream,
                        None => snapshot_upload_stream(
                            zfs_snapshot_mount_get(
                                &self.config.zfs_dataset_name,
                                &backup_step_upload.snapshot_name,
                            )
                            .await?
                            .ok_or(anyhow!("No zfs mountpoint"))?,
                            // Unfortunately we have to clone the whole thing
                            backup_step_upload.diff.shallow_clone().into_owned(),
                            backup_step_upload.uploaded_objects * max_object_size,
                        )
                        .try_chunks_streams(),
                    };

                    // For testing interrupted uploading
                    sleep_with_spinner(Duration::from_secs(3)).await;
                    let object_len = (snapshot_upload_size
                        - backup_step_upload.uploaded_objects * max_object_size)
                        .min(max_object_size);
                    let mut spinner = Spinner::with_timer(
                        Spinners::Dots,
                        format!(
                            "Uploading part {} ({})",
                            backup_step_upload.uploaded_objects,
                            format_size(object_len, DECIMAL)
                        ),
                    );
                    s3_client
                        .put_object()
                        // TODO: Deep Archive
                        .storage_class(StorageClass::Standard)
                        .bucket(self.backup_data.s3_bucket.as_ref())
                        .key(format!(
                            "{}/{}/{}",
                            SNAPSHOTS_PREFIX,
                            backup_step_upload.snapshot_name,
                            backup_step_upload.uploaded_objects
                        ))
                        .if_none_match("*")
                        .content_length(object_len as i64)
                        .body({
                            ByteStream::from_body_1_x(reqwest::Body::wrap_stream(
                                snapshot_upload_stream.take_bytes_stream(max_object_size as usize),
                            ))
                        })
                        .send()
                        .await
                        .map_or_else(
                            |e| {
                                match &e {
                                    SdkError::ServiceError(service_error) => {
                                        if service_error.raw().status().as_u16() == 412 {
                                            return Ok(());
                                        }
                                    }
                                    _ => {}
                                };
                                Err(anyhow::Error::from(e))
                            },
                            |_| Ok(()),
                        )?;

                    spinner.stop_with_newline();

                    backup_step_upload.uploaded_objects += 1;
                    Some(snapshot_upload_stream)
                } else {
                    None
                };

                Ok(RetryStepOutput2::NotFinished(
                    if backup_step_upload.uploaded_objects == total_objects_count {
                        RetryStepNotFinished2 {
                            memory_data: None,
                            persistent_data: backup_step_upload.next(),
                        }
                    } else {
                        RetryStepNotFinished2 {
                            memory_data: snapshot_upload_stream,
                            persistent_data: BackupStep::Upload(backup_step_upload),
                        }
                    },
                ))
            }
            BackupStep::UpdateHotData(backup_step_upload_hot_data) => {
                let mut spinner = Spinner::with_timer(Spinners::Dots, "Updating hot data".into());
                let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
                let s3_client = aws_sdk_s3::Client::new(&sdk_config);
                let snapshot_name = backup_step_upload_hot_data.snapshot_name;
                // Only update if we have to
                let remote_hot_data = self.take_remote_hot_data(&s3_client).await?;
                if remote_hot_data
                    .snapshots
                    .last()
                    .map(|snapshot| snapshot.deref())
                    != Some(snapshot_name.deref())
                {
                    let new_hot_data = RemoteHotDataDecrypted {
                        snapshots: {
                            let mut s = remote_hot_data.snapshots.shallow_clone();
                            s.push(snapshot_name.shallow_clone());
                            s
                        },
                        ..remote_hot_data
                    };
                    upload_hot_data(
                        &self.config,
                        &s3_client,
                        &self.backup_data.s3_bucket,
                        &new_hot_data,
                    )
                    .await?;
                }
                spinner.stop_with_newline();
                Ok(RetryStepOutput2::Finished(Some(snapshot_name)))
            }
        }
    }
}
