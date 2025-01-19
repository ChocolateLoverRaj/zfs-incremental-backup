use std::{io::BufRead, path::PathBuf};

use anyhow::anyhow;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

/// Based on https://openzfs.github.io/openzfs-docs/man/master/8/zfs-diff.8.html, but only the types relevant to backups
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
pub enum FileType {
    Directory,
    RegularFile,
}

/// Based on https://openzfs.github.io/openzfs-docs/man/master/8/zfs-diff.8.html, but more Rust friendly
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum DiffType<T> {
    Removed,
    Created(T),
    Modified(T),
    Renamed(PathBuf),
}

impl<T> DiffType<T> {
    pub fn map<N>(self, mut f: impl FnMut(T) -> N) -> DiffType<N> {
        match self {
            DiffType::Created(prev) => DiffType::Created(f(prev)),
            DiffType::Modified(prev) => DiffType::Modified(f(prev)),
            DiffType::Renamed(a) => DiffType::Renamed(a),
            DiffType::Removed => DiffType::Removed,
        }
    }

    #[allow(unused)]
    pub async fn map_async<N>(self, mut f: impl FnMut(T) -> BoxFuture<'static, N>) -> DiffType<N> {
        match self {
            DiffType::Created(prev) => DiffType::Created(f(prev).await),
            DiffType::Modified(prev) => DiffType::Modified(f(prev).await),
            DiffType::Renamed(a) => DiffType::Renamed(a),
            DiffType::Removed => DiffType::Removed,
        }
    }

    pub async fn try_map_async<N, E>(
        self,
        mut f: impl FnMut(T) -> BoxFuture<'static, Result<N, E>>,
    ) -> Result<DiffType<N>, E> {
        Ok(match self {
            DiffType::Created(prev) => DiffType::Created(f(prev).await?),
            DiffType::Modified(prev) => DiffType::Modified(f(prev).await?),
            DiffType::Renamed(a) => DiffType::Renamed(a),
            DiffType::Removed => DiffType::Removed,
        })
    }

    pub fn content_data(&self) -> Option<&T> {
        match self {
            DiffType::Created(content_data) => Some(content_data),
            DiffType::Modified(content_data) => Some(content_data),
            _ => None,
        }
    }
}

/// Based on https://openzfs.github.io/openzfs-docs/man/master/8/zfs-diff.8.html
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct DiffEntry<T> {
    pub path: PathBuf,
    pub file_type: FileType,
    pub diff_type: DiffType<T>,
}

impl<T> DiffEntry<T> {
    pub fn map<N>(self, f: impl FnMut(T) -> N) -> DiffEntry<N> {
        DiffEntry {
            path: self.path,
            file_type: self.file_type,
            diff_type: self.diff_type.map(f),
        }
    }

    #[allow(unused)]
    pub async fn map_async<N>(self, f: impl FnMut(T) -> BoxFuture<'static, N>) -> DiffEntry<N> {
        DiffEntry {
            path: self.path,
            file_type: self.file_type,
            diff_type: self.diff_type.map_async(f).await,
        }
    }

    pub async fn try_map_async<N, E>(
        self,
        f: impl FnMut(T) -> BoxFuture<'static, Result<N, E>>,
    ) -> Result<DiffEntry<N>, E> {
        Ok(DiffEntry {
            path: self.path,
            file_type: self.file_type,
            diff_type: self.diff_type.try_map_async(f).await?,
        })
    }
}

impl DiffEntry<()> {
    pub fn from_zfs_diff_line(line: &str) -> anyhow::Result<Option<Self>> {
        let columns = line.split('\t').collect::<Vec<_>>();
        let path = *columns.get(2).ok_or(anyhow!("Empty file path column"))?;
        // TODO: Store xattr and permissions stuff
        if path.contains("<xattrdir>") {
            Ok(None)
        } else {
            Ok(Some(DiffEntry {
                path: path.into(),
                diff_type: match *columns.get(0).ok_or(anyhow!("Empty line"))? {
                    "-" => Ok(DiffType::Removed),
                    "+" => Ok(DiffType::Created(())),
                    "M" => Ok(DiffType::Modified(())),
                    "R" => Ok(DiffType::Renamed({
                        columns.get(3).ok_or(anyhow!("No renamed path"))?.into()
                    })),
                    _ => Err(anyhow!("Unexpected diff type")),
                }?,
                file_type: match *columns.get(1).ok_or(anyhow!("Empty file type column"))? {
                    "/" => Ok(FileType::Directory),
                    "F" => Ok(FileType::RegularFile),
                    file_type => Err(anyhow!("Unexpected file type: {:?}", file_type)),
                }?,
            }))
        }
    }
}

pub fn parse_zfs_diff_output(output: Vec<u8>) -> anyhow::Result<Vec<DiffEntry<()>>> {
    let diff_entries = output
        .lines()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|line| DiffEntry::from_zfs_diff_line(&line))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|o| o)
        .collect();
    Ok(diff_entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zfs_diff() {
        let parsed_diff = parse_zfs_diff_output(
            [
                "+\tF\t/mnt/long-term-files/created_after_snapshot_0.txt",
                "M\t/\t/mnt/long-term-files/",
                "R\tF\t/mnt/long-term-files/file with spaces.txt\t/mnt/long-term-files/moved after snapshot 2.txt",
                "+\t/\t/mnt/long-term-files/folder",
                "+\t/\t/mnt/long-term-files/folder/<xattrdir>",
                "+\tF\t/mnt/long-term-files/folder/<xattrdir>/system.posix_acl_default"
            ]
            .join("\n")
            .as_bytes()
            .to_vec(),
        )
        .unwrap();
        let expected = vec![
            DiffEntry {
                path: "/mnt/long-term-files/created_after_snapshot_0.txt".into(),
                file_type: FileType::RegularFile,
                diff_type: DiffType::Created(()),
            },
            DiffEntry {
                path: "/mnt/long-term-files/".into(),
                file_type: FileType::Directory,
                diff_type: DiffType::Modified(()),
            },
            DiffEntry {
                path: "/mnt/long-term-files/file with spaces.txt".into(),
                file_type: FileType::RegularFile,
                diff_type: DiffType::Renamed(
                    "/mnt/long-term-files/moved after snapshot 2.txt".into(),
                ),
            },
            DiffEntry {
                path: "/mnt/long-term-files/folder".into(),
                file_type: FileType::Directory,
                diff_type: DiffType::Created(()),
            },
        ];
        assert_eq!(parsed_diff, expected);
    }
}
