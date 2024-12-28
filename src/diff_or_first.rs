use std::{io::BufRead, path::PathBuf};

use anyhow::anyhow;
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::{read_dir_recursive::read_dir_recursive, zfs_mount_get::zfs_mount_get};

/// Based on https://openzfs.github.io/openzfs-docs/man/master/8/zfs-diff.8.html, but only the types relevant to backups
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
enum FileType {
    Directory,
    RegularFile,
}

/// Based on https://openzfs.github.io/openzfs-docs/man/master/8/zfs-diff.8.html, but more Rust friendly
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
enum DiffType {
    Removed,
    Created,
    Modified,
    Renamed(PathBuf),
}

/// Based on https://openzfs.github.io/openzfs-docs/man/master/8/zfs-diff.8.html
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffEntry {
    path: PathBuf,
    file_type: FileType,
    diff_type: DiffType,
}

impl DiffEntry {
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
                    "+" => Ok(DiffType::Created),
                    "M" => Ok(DiffType::Modified),
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

fn parse_zfs_diff_output(output: Vec<u8>) -> anyhow::Result<Vec<DiffEntry>> {
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
pub async fn diff_or_first(
    dataset: String,
    previous_snapshot: Option<&str>,
    recent_snapshot: String,
) -> anyhow::Result<Vec<DiffEntry>> {
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
        let diff_entries = parse_zfs_diff_output(command.stdout)?;
        Ok(diff_entries)
    } else {
        let mount_point = zfs_mount_get(&dataset)
            .await?
            .ok_or(anyhow!("dataset not mounted"))?;
        // TODO: This could be improved by reading folders parallel
        // let mount_point = {
        //     let output = Command::new("zfs")
        //         .arg("get")
        //         .arg("mountpoint")
        //         .arg(dataset)
        //         .arg("-H")
        //         .arg("-o")
        //         .arg("value")
        //         .output()
        //         .await?;
        //     if !output.status.success() {
        //         Err(anyhow!("zfs get failed"))?;
        //     }
        //     let mount_point = PathBuf::from(String::from_utf8(output.stdout)?.trim());

        // };
        println!("Got mountpoint: {mount_point:?}");
        let files = read_dir_recursive(mount_point)
            .map(|(path, result)| result.map(|(dir_entry, file_type)| (path, dir_entry, file_type)))
            .try_collect::<Vec<_>>()
            .await?;
        let diff_entries = files
            .into_iter()
            .map(|(path, _dir_entry, file_type)| {
                anyhow::Ok(DiffEntry {
                    path,
                    diff_type: DiffType::Created,
                    file_type: if file_type.is_file() {
                        Ok(FileType::RegularFile)
                    } else if file_type.is_dir() {
                        Ok(FileType::Directory)
                    } else {
                        Err(anyhow!("Cannot handle file type: {:?}", file_type))
                    }?,
                })
            })
            .collect::<Result<_, _>>()?;
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
                diff_type: DiffType::Created,
            },
            DiffEntry {
                path: "/mnt/long-term-files/".into(),
                file_type: FileType::Directory,
                diff_type: DiffType::Modified,
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
                diff_type: DiffType::Created,
            },
        ];
        assert_eq!(parsed_diff, expected);
    }
}
