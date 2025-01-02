use std::path::PathBuf;

use proc_mounts::MountIter;

/// Remember that this method does not get a mounted read-only snapshot
pub async fn zfs_mount_get(dataset: &str) -> anyhow::Result<Option<PathBuf>> {
    // TODO: Actually async file reading
    Ok(
        MountIter::new_from_file("/proc/self/mounts")?.find_map(|mount| match mount {
            Ok(mount) => {
                if mount.fstype == "zfs" && mount.source.to_str() == Some(dataset) {
                    Some(mount.dest)
                } else {
                    None
                }
            }
            Err(_) => None,
        }),
    )
}

pub async fn zfs_snapshot_mount_get(
    dataset: &str,
    snapshot: &str,
) -> anyhow::Result<Option<PathBuf>> {
    Ok(zfs_mount_get(dataset)
        .await?
        .map(|mut dataset_mount_point| {
            dataset_mount_point.push(".zfs/snapshot");
            dataset_mount_point.push(snapshot);
            dataset_mount_point
        }))
}
