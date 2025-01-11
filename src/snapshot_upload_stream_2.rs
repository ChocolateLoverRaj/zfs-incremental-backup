use std::{io, path::PathBuf};

use bytes::Bytes;
use futures::{stream, FutureExt, Stream, StreamExt, TryFutureExt};
use tokio::{fs::File, io::AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::{diff_entry::DiffEntry, file_meta_data::FileMetaData};

pub fn snapshot_upload_stream(
    mount_point: PathBuf,
    diff_entries: Vec<DiffEntry<Option<FileMetaData>>>,
    mut start_index: u64,
) -> impl Stream<Item = io::Result<Bytes>> {
    stream::iter(diff_entries).flat_map(move |diff_entry| {
        match postcard::to_allocvec(&diff_entry) {
            Ok(postcard_bytes) => {
                let mut sync_chunks = Vec::<io::Result<Bytes>>::new();
                {
                    let postcard_size_bytes = {
                        let (bytes, len) = varint_simd::encode(postcard_bytes.len() as u64);
                        bytes[..len as usize].to_vec()
                    };
                    let skip_bytes = start_index.min(postcard_size_bytes.len() as u64);
                    start_index -= skip_bytes;
                    if skip_bytes < postcard_bytes.len() as u64 {
                        sync_chunks.push(Ok(Bytes::copy_from_slice(
                            &postcard_size_bytes[skip_bytes as usize..],
                        )));
                    }
                }
                {
                    let skip_bytes = start_index.min(postcard_bytes.len() as u64);
                    start_index -= skip_bytes;
                    if skip_bytes < postcard_bytes.len() as u64 {
                        sync_chunks.push(Ok(Bytes::from_owner(postcard_bytes)));
                    }
                }
                let s = stream::iter(sync_chunks);
                match diff_entry.diff_type.content_data().copied().flatten() {
                    Some(file_meta_data) => {
                        if start_index > 0 {
                            if start_index >= file_meta_data.len {
                                start_index -= file_meta_data.len;
                                s.boxed()
                            } else {
                                s.chain(
                                    {
                                        let path = mount_point.join(&diff_entry.path);
                                        async move {
                                            let mut file = File::open(path).await?;
                                            file.seek(io::SeekFrom::Start(start_index)).await?;
                                            Ok(ReaderStream::new(file))
                                        }
                                    }
                                    .try_flatten_stream(),
                                )
                                .boxed()
                            }
                        } else {
                            s.chain(
                                File::open(mount_point.join(&diff_entry.path))
                                    .map(|result| result.map(|file| ReaderStream::new(file)))
                                    .try_flatten_stream(),
                            )
                            .boxed()
                        }
                    }
                    None => s.boxed(),
                }
            }
            Err(e) => stream::iter([Err(io::Error::other(e))]).boxed(),
        }
    })
}
