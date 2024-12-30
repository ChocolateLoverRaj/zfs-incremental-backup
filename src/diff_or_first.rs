use std::{io::BufRead, path::PathBuf};

use anyhow::anyhow;
use futures::{future::BoxFuture, stream, FutureExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::{read_dir_recursive::read_dir_recursive, zfs_mount_get::zfs_mount_get};

/// Based on https://openzfs.github.io/openzfs-docs/man/master/8/zfs-diff.8.html, but only the types relevant to backups
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
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

fn parse_zfs_diff_output(output: Vec<u8>) -> anyhow::Result<Vec<DiffEntry<()>>> {
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

/// Parses the output of zfs diff or reads all the files if there is no previous snapshot to compare to
/// Does not include "modified folder" because it will include the actual modifications within the folder anyways
/// Always sorted in order of folders before stuff inside folders
pub async fn diff_or_first(
    dataset: &str,
    previous_snapshot: Option<&str>,
    recent_snapshot: &str,
) -> anyhow::Result<Vec<DiffEntry<Option<u64>>>> {
    if let Some(previous_snapshot) = previous_snapshot {
        let command = Command::new("zfs")
            .arg("diff")
            // Use h to properly parse files with spaces in their names. Columns are tab seperated.
            .arg("-FHh")
            .arg(format!("{}@{}", dataset, previous_snapshot))
            .arg(format!("{}@{}", dataset, recent_snapshot))
            .output()
            .await?;
        if !command.status.success() {
            Err(anyhow!("zfs diff failed"))?;
        }
        let mut diff_entries = parse_zfs_diff_output(command.stdout)?
            .into_iter()
            .filter(|diff| {
                if diff.file_type == FileType::Directory && diff.diff_type == DiffType::Modified(())
                {
                    false
                } else {
                    true
                }
            })
            .map(|entry| entry.map(|()| None))
            .collect::<Vec<_>>();
        // Sort it by path so that folders come before their children
        diff_entries.sort_by_key(|diff| diff.path.clone());
        Ok(diff_entries)
    } else {
        let mount_point = zfs_mount_get(&dataset)
            .await?
            .ok_or(anyhow!("dataset not mounted"))?;
        println!("Got mountpoint: {mount_point:?}");
        let files = read_dir_recursive(mount_point.clone())
            .map(|(path, result)| result.map(|(dir_entry, file_type)| (path, dir_entry, file_type)))
            .try_collect::<Vec<_>>()
            .await?;
        let diff_entries = stream::iter(files.into_iter())
            .flat_map_unordered(None, |(path, dir_entry, file_type)| {
                let mount_point = mount_point.clone();
                async move {
                    let file_type = if file_type.is_file() {
                        Ok(FileType::RegularFile)
                    } else if file_type.is_dir() {
                        Ok(FileType::Directory)
                    } else {
                        Err(anyhow!("Cannot handle file type: {:?}", file_type))
                    }?;
                    anyhow::Ok(DiffEntry {
                        path: path.strip_prefix(&mount_point)?.into(),
                        diff_type: DiffType::Created(match file_type {
                            FileType::RegularFile => Some(dir_entry.metadata().await?.len()),
                            FileType::Directory => None,
                        }),
                        file_type,
                    })
                }
                .into_stream()
                .boxed()
            })
            .try_collect::<Vec<_>>()
            .await?;
        Ok(diff_entries)
    }
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
