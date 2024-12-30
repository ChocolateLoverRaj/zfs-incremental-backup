use std::{
    io::{self, SeekFrom},
    path::PathBuf,
};

use anyhow::anyhow;
use futures::{AsyncRead, AsyncSeek, AsyncSeekExt, FutureExt};
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncReadExt,
};

use crate::diff_or_first::DiffEntry;

enum ReadDiffEntryState {
    PostcardSize(u8),
    PostcardData(u64),
    Content(Option<File>),
}

impl ReadDiffEntryState {
    fn current_pos_bytes(&self, diff_entry: &DiffEntry<Option<u64>>) -> anyhow::Result<u64> {
        Ok(match self {
            Self::PostcardSize(index) => *index as u64,
            Self::PostcardData(index) => {
                let postcard_len = postcard::to_allocvec(diff_entry)
                    .map_err(|e| io::Error::other(e))?
                    .len() as u64;
                let postcard_size_len = varint_simd::encode(postcard_len).1 as u64;
                postcard_size_len + *index
            }
            Self::Content(_) => Err(anyhow!("not implemented for tokio File"))?,
        })
    }

    pub async fn seek_forward(
        &mut self,
        mount_point: &PathBuf,
        diff_entry: &DiffEntry<Option<u64>>,
        mut forward_by: u64,
    ) -> anyhow::Result<(u64, u64)> {
        Ok((
            {
                loop {
                    match self {
                        Self::PostcardSize(index) => {
                            let postcard_len = postcard::to_allocvec(diff_entry)
                                .map_err(|e| io::Error::other(e))?
                                .len() as u64;
                            let postcard_size_len = varint_simd::encode(postcard_len).1 as u64;
                            if forward_by < postcard_size_len - *index as u64 {
                                *index += forward_by as u8;
                                break self.current_pos_bytes(diff_entry)?;
                            } else {
                                forward_by -= postcard_size_len - *index as u64;
                                *self = Self::PostcardData(0);
                            }
                        }
                        Self::PostcardData(index) => {
                            let postcard_len = postcard::to_allocvec(diff_entry)
                                .map_err(|e| io::Error::other(e))?
                                .len() as u64;
                            if forward_by < postcard_len - *index {
                                *index += forward_by;
                                break self.current_pos_bytes(diff_entry)?;
                            } else {
                                forward_by -= postcard_len - *index;
                                *self = Self::Content(None);
                            }
                        }
                        Self::Content(file) => {
                            let file = match file {
                                None => file.insert(
                                    OpenOptions::new()
                                        .read(true)
                                        .write(false)
                                        .open(mount_point.join(&diff_entry.path))
                                        .await?,
                                ),
                                Some(file) => file,
                            };
                            let postcard_data = postcard::to_allocvec(diff_entry)
                                .map_err(|e| io::Error::other(e))?;
                            let postcard_data_len = postcard_data.len() as u64;
                            let postcard_size_len = varint_simd::encode(postcard_data_len).1 as u64;
                            let content_pos = tokio::io::AsyncSeekExt::seek(
                                file,
                                SeekFrom::Current(forward_by as i64),
                            )
                            .await?;
                            forward_by -= content_pos;
                            break postcard_size_len + postcard_data_len + content_pos;
                        }
                    }
                }
            },
            forward_by,
        ))
    }
}

struct DiffEntryPosition {
    diff_entry_index: usize,
    state: ReadDiffEntryState,
}

enum PositionState {
    ReadDiffEntry(DiffEntryPosition),
    End,
}

impl PositionState {
    pub fn start(diff_entries: &Vec<DiffEntry<Option<u64>>>) -> Self {
        match diff_entries.len() {
            1.. => Self::ReadDiffEntry(DiffEntryPosition {
                diff_entry_index: 0,
                state: ReadDiffEntryState::PostcardSize(0),
            }),
            0 => Self::End,
        }
    }
}

/// Attempting to seek beyond the end will just move it to the end
pub struct SnapshotUploadStream {
    mount_point: PathBuf,
    diff_entries: Vec<DiffEntry<Option<u64>>>,
    position_state: PositionState,
}

fn get_diff_entry_size(diff_entry: &DiffEntry<Option<u64>>) -> postcard::Result<u64> {
    let postcard_len = postcard::to_allocvec(diff_entry)?.len() as u64;
    Ok(
        // Length of the postcard
        varint_simd::encode(postcard_len).1 as u64
        // Postcard also contain length of content
        + postcard_len
        // The content (for create / modify)
        + diff_entry.diff_type.content_data().copied().flatten().unwrap_or(0),
    )
}

impl SnapshotUploadStream {
    pub fn new(mount_point: PathBuf, diff_entries: Vec<DiffEntry<Option<u64>>>) -> Self {
        Self {
            mount_point,
            position_state: PositionState::start(&diff_entries),
            diff_entries,
        }
    }

    pub fn get_size(&self) -> postcard::Result<u64> {
        Ok(self.diff_entries.iter().try_fold(0, |sum, diff_entry| {
            Ok(sum + get_diff_entry_size(diff_entry)?)
        })?)
    }

    // /// Gets the current position in bytes
    // pub fn current_position_bytes(&self) -> postcard::Result<u64> {
    //     let (diff_entries, position_within_diff_entry) = match self.position_state {
    //         PositionState::ReadDiffEntry(diff_entry_position) => (
    //             &self.diff_entries[..diff_entry_position.diff_entry_index],
    //             {
    //                 match diff_entry_position.state {
    //                     ReadDiffEntryState::PostcardSize(index) => index as u64,
    //                     ReadDiffEntryState::PostcardData(index) => {
    //                         let postcard_data = postcard::to_allocvec(
    //                             &self.diff_entries[diff_entry_position.diff_entry_index],
    //                         )?;
    //                         varint_simd::encode(postcard_data.len() as u64).1 as u64 + index
    //                     }
    //                     ReadDiffEntryState::Content(file) => match file {
    //                         Some(file) => file.stream_position(),
    //                         None => 0,
    //                     },
    //                 }
    //             },
    //         ),
    //         PositionState::End => (self.diff_entries.as_slice(), 0),
    //     };
    //     Ok(diff_entries.iter().try_fold(0, |sum, diff_entry| {
    //         Ok(sum + get_diff_entry_size(diff_entry)?)
    //     })? + position_within_diff_entry)
    // }
}

impl AsyncSeek for SnapshotUploadStream {
    fn poll_seek(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        pos: io::SeekFrom,
    ) -> std::task::Poll<io::Result<u64>> {
        async move {
            let s = self.get_mut();
            Ok(match pos {
                SeekFrom::Start(index) => {
                    s.position_state = PositionState::start(&s.diff_entries);
                    s.seek(SeekFrom::Current(index as i64)).await?
                }
                SeekFrom::Current(index) => {
                    if index.is_negative() {
                        Err(io::Error::other(anyhow!(
                            "Seeking backwards not implemented"
                        )))?;
                    }
                    match &mut s.position_state {
                        PositionState::ReadDiffEntry(diff_entry_position) => {
                            let mut len = s.diff_entries[..diff_entry_position.diff_entry_index]
                                .iter()
                                .try_fold(0, |sum, diff_entry| {
                                    postcard::Result::Ok(sum + get_diff_entry_size(diff_entry)?)
                                })
                                .map_err(|e| io::Error::other(e))?;
                            loop {
                                let (position, remaining) = diff_entry_position
                                    .state
                                    .seek_forward(
                                        &s.mount_point,
                                        &s.diff_entries[diff_entry_position.diff_entry_index],
                                        index as u64,
                                    )
                                    .await
                                    .map_err(|e| io::Error::other(e))?;
                                len += position;
                                if remaining == 0 {
                                    break len;
                                } else {
                                    diff_entry_position.diff_entry_index += 1;
                                    if diff_entry_position.diff_entry_index == s.diff_entries.len()
                                    {
                                        break s.get_size().map_err(|e| io::Error::other(e))?;
                                    }
                                }
                            }
                        }
                        PositionState::End => s.get_size().map_err(|e| io::Error::other(e))?,
                    }
                }
                SeekFrom::End(index) => {
                    if index.is_negative() {
                        Err(io::Error::other(anyhow!(
                            "Seeking backwards not implemented"
                        )))?;
                    }
                    s.position_state = PositionState::End;
                    s.get_size().map_err(|e| io::Error::other(e))?
                }
            })
        }
        .boxed_local()
        .poll_unpin(cx)
    }
}

impl AsyncRead for SnapshotUploadStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        async move {
            let s = self.get_mut();
            Ok(loop {
                match &mut s.position_state {
                    PositionState::ReadDiffEntry(diff_entry_position) => {
                        match &mut diff_entry_position.state {
                            ReadDiffEntryState::PostcardSize(index) => {
                                let postcard_len = postcard::to_allocvec(
                                    &s.diff_entries[diff_entry_position.diff_entry_index],
                                )
                                .map_err(|e| io::Error::other(e))?
                                .len() as u64;
                                break if *index == 0 && buf.len() >= 10 {
                                    let size =
                                        varint_simd::encode_to_slice(postcard_len, buf) as usize;
                                    diff_entry_position.state = ReadDiffEntryState::PostcardData(0);
                                    size
                                } else {
                                    let (len_buf, len_buf_len) = varint_simd::encode(postcard_len);
                                    let copy_len = (len_buf_len - *index).min(buf.len() as u8);
                                    buf[..copy_len as usize].copy_from_slice(
                                        &len_buf[*index as usize..copy_len as usize],
                                    );
                                    *index += copy_len;
                                    if *index == len_buf_len {
                                        diff_entry_position.state =
                                            ReadDiffEntryState::PostcardData(0);
                                    }
                                    copy_len as usize
                                };
                            }
                            ReadDiffEntryState::PostcardData(index) => {
                                let postcard_data = postcard::to_allocvec(
                                    &s.diff_entries[diff_entry_position.diff_entry_index],
                                )
                                .map_err(|e| io::Error::other(e))?;
                                let copy_len =
                                    (postcard_data.len() - *index as usize).max(buf.len());
                                buf[..copy_len].copy_from_slice(&postcard_data);
                                *index += copy_len as u64;
                                if *index as usize == postcard_data.len() {
                                    diff_entry_position.state = ReadDiffEntryState::Content(None);
                                }
                                break copy_len;
                            }
                            ReadDiffEntryState::Content(file) => {
                                let file = match file {
                                    None => file.insert(
                                        OpenOptions::new()
                                            .read(true)
                                            .write(false)
                                            .open(
                                                s.mount_point.join(
                                                    &s.diff_entries
                                                        [diff_entry_position.diff_entry_index]
                                                        .path,
                                                ),
                                            )
                                            .await?,
                                    ),
                                    Some(file) => file,
                                };
                                let len = file.read(buf).await?;
                                if len != 0 {
                                    break len;
                                } else {
                                    diff_entry_position.diff_entry_index += 1;
                                    if diff_entry_position.diff_entry_index < s.diff_entries.len() {
                                        diff_entry_position.state =
                                            ReadDiffEntryState::PostcardSize(0);
                                    } else {
                                        s.position_state = PositionState::End;
                                    }
                                }
                            }
                        }
                    }
                    PositionState::End => {
                        // We're done
                        break 0;
                    }
                }
            })
        }
        .boxed_local()
        .poll_unpin(cx)
    }
}
