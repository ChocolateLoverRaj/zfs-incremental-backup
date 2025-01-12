use std::fs::Metadata;

use anyhow::anyhow;
use futures::{stream, FutureExt, StreamExt, TryStreamExt};
use tokio::process::Command;

use crate::{
    diff_entry::{parse_zfs_diff_output, DiffEntry, DiffType, FileType},
    read_dir_recursive::read_dir_recursive,
    zfs_mount_get::{zfs_get_snapshot_path, zfs_mount_get},
};

/// Parses the output of zfs diff or reads all the files if there is no previous snapshot to compare to
/// Does not include "modified folder" because it will include the actual modifications within the folder anyways
pub async fn diff_or_first(
    dataset: &str,
    previous_snapshot: Option<&str>,
    recent_snapshot: &str,
) -> anyhow::Result<Vec<DiffEntry<Option<Metadata>>>> {
    let zfs_mount_point = zfs_mount_get(dataset)
        .await?
        .ok_or(anyhow!("dataset not mounted"))?;
    let snapshot_mount_point = zfs_get_snapshot_path(zfs_mount_point.clone(), recent_snapshot);
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
            Err(anyhow!(
                "zfs diff failed: {:?}. Do the snapshots exist? Are you trying to compare the same snapshot with itself?",
                String::from_utf8(command.stderr)
            ))?;
        }
        let diff_entries = parse_zfs_diff_output(command.stdout)?
            .into_iter()
            // TODO: More optimizing
            .filter(|diff| {
                if diff.file_type == FileType::Directory && diff.diff_type == DiffType::Modified(())
                {
                    false
                } else {
                    true
                }
            })
            .map(|entry| entry.map(|()| None))
            .map(|mut entry| {
                anyhow::Ok({
                    entry.path = entry.path.strip_prefix(&zfs_mount_point)?.into();
                    entry.diff_type = match entry.diff_type {
                        DiffType::Renamed(new_path) => {
                            DiffType::Renamed(new_path.strip_prefix(&zfs_mount_point)?.into())
                        }
                        diff_type => diff_type,
                    };
                    entry
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(diff_entries)
    } else {
        println!("Got mountpoint: {snapshot_mount_point:?}");
        let files = read_dir_recursive(snapshot_mount_point.clone())
            .map(|(path, result)| result.map(|(dir_entry, file_type)| (path, dir_entry, file_type)))
            .try_collect::<Vec<_>>()
            .await?;
        let diff_entries = stream::iter(files.into_iter())
            .flat_map_unordered(None, |(path, dir_entry, file_type)| {
                let mount_point = snapshot_mount_point.clone();
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
                            FileType::RegularFile => Some(dir_entry.metadata().await?),
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
