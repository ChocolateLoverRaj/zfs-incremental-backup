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
        .map(|dataset_mount_point| zfs_get_snapshot_path(dataset_mount_point, snapshot)))
}

pub fn zfs_get_snapshot_path(mut zfs_path: PathBuf, snapshot: &str) -> PathBuf {
    zfs_path.push(".zfs/snapshot");
    zfs_path.push(snapshot);
    zfs_path
}
