use std::path::PathBuf;

use proc_mounts::MountIter;

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
