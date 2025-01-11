// TODO: Rename this to take_bytes_many_times or something
use std::sync::Arc;

use bytes::Bytes;
use futures::{
    lock::Mutex,
    stream::{self, BoxStream},
    FutureExt, Stream, StreamExt,
};

pub trait ChunksStreamExt<'a, T>
where
    Self: Sized,
{
    fn try_chunks_streams(self) -> ChunksStreamOfStreams<'a, T>;
}

impl<'a, S, T> ChunksStreamExt<'a, T> for S
where
    S: Stream<Item = T> + Send + 'a,
{
    fn try_chunks_streams(self) -> ChunksStreamOfStreams<'a, T> {
        ChunksStreamOfStreams {
            inner: Arc::new(Mutex::new(ChunksStreamOfStreamsInner {
                stream: self.boxed(),
                buffer: Default::default(),
            })),
        }
    }
}

struct ChunksStreamOfStreamsInner<'a, T> {
    stream: BoxStream<'a, T>,
    buffer: Option<Bytes>,
}

pub struct ChunksStreamOfStreams<'a, T> {
    inner: Arc<Mutex<ChunksStreamOfStreamsInner<'a, T>>>,
}
impl<'a, T> Clone for ChunksStreamOfStreams<'a, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, E> ChunksStreamOfStreams<'a, Result<Bytes, E>>
where
    E: 'a,
{
    pub fn take_bytes_stream(&self, n_bytes: usize) -> BoxStream<'a, Result<Bytes, E>> {
        self.inner
            .clone()
            .lock_owned()
            .boxed()
            .map(move |inner| {
                stream::unfold((inner, 0), move |(mut inner, count)| async move {
                    if count == n_bytes {
                        None
                    } else if let Some(buffer) = inner.buffer.take() {
                        let buffer_len = buffer.len();
                        Some((Ok(buffer), (inner, count + buffer_len)))
                    } else {
                        match inner.stream.next().await {
                            Some(result) => Some(match result {
                                Ok(mut bytes) => {
                                    let bytes_len = bytes.len();
                                    if count + bytes_len >= n_bytes {
                                        let remaining = bytes.split_off(n_bytes - count);
                                        inner.buffer = Some(remaining);
                                        let bytes_len = bytes.len();
                                        (Ok(bytes), (inner, count + bytes_len))
                                    } else {
                                        (Ok(bytes), (inner, count + bytes_len))
                                    }
                                }
                                Err(e) => (Err(e), (inner, count)),
                            }),
                            None => None,
                        }
                    }
                })
            })
            .flatten_stream()
            .boxed()
    }
}
