use std::{fs::FileType, path::PathBuf};

use futures::{
    stream::{self, BoxStream},
    FutureExt, StreamExt,
};
use tokio::{
    fs::{read_dir, DirEntry},
    io,
};
use tokio_stream::wrappers::ReadDirStream;

pub fn read_dir_recursive(
    path: PathBuf,
) -> BoxStream<'static, (PathBuf, io::Result<(DirEntry, FileType)>)> {
    println!("Reading dir: {path:?}");
    read_dir(path.clone())
        .map({
            let path = path.clone();
            move |result| match result {
                Ok(dir) => ReadDirStream::new(dir)
                    .flat_map_unordered(None, {
                        let path = path.clone();
                        move |result| match result {
                            Ok(dir_entry) => async move {
                                let file_type = dir_entry.file_type().await;
                                (dir_entry, file_type)
                            }
                            .map(|(dir_entry, result)| match result {
                                Ok(file_type) => {
                                    let path = dir_entry.path();
                                    stream::iter(vec![(path.clone(), Ok((dir_entry, file_type)))])
                                        .chain(if file_type.is_dir() {
                                            read_dir_recursive(path).boxed()
                                        } else {
                                            stream::empty().boxed()
                                        })
                                        .boxed()
                                }
                                Err(e) => {
                                    futures::stream::iter(vec![(dir_entry.path(), Err(e))]).boxed()
                                }
                            })
                            .flatten_stream()
                            .boxed(),
                            Err(e) => futures::stream::iter(vec![(path.clone(), Err(e))]).boxed(),
                        }
                    })
                    .boxed(),
                Err(e) => futures::stream::iter(vec![(path.clone(), Err(e))]).boxed(),
            }
        })
        .flatten_stream()
        .boxed()
}
