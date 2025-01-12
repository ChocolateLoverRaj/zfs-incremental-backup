use std::fmt::Debug;

use crate::diff_entry::{DiffEntry, DiffType, FileType};

/// Removes unnecessary diff entries:
/// - Removes modified folders. The actual modification within the folder is all we need.
/// - Removes created folders if files were created within the folder. We can automatically create parent folders of files when restoring.
/// - Removes deleted files if the folder they are located in was deleted
pub fn optimize_diff_entries<T: Debug>(diff_entries: &mut Vec<DiffEntry<T>>) {
    // Sorting helps speed up finding files inside folders
    diff_entries.sort_by_key(|diff| diff.path.clone());
    let mut i = 0;
    loop {
        match diff_entries.get(i) {
            Some(diff_entry) => match diff_entry.file_type {
                FileType::Directory => match diff_entry.diff_type {
                    DiffType::Modified(_) => {
                        diff_entries.remove(i);
                    }
                    DiffType::Created(_) => {
                        let folder_entry = diff_entry;
                        let folder_is_empty = match diff_entries.get(i + 1) {
                            Some(diff_entry) => {
                                if diff_entry.path.starts_with(&folder_entry.path) {
                                    false
                                } else {
                                    true
                                }
                            }
                            None => true,
                        };
                        if !folder_is_empty {
                            diff_entries.remove(i);
                        }
                    }
                    _ => {}
                },
                FileType::RegularFile => match diff_entry.diff_type {
                    DiffType::Removed => {
                        let parent_folder_removed = match i.checked_sub(1) {
                            Some(i) => match diff_entries.get(i) {
                                Some(maybe_parent_folder) => {
                                    if maybe_parent_folder.file_type == FileType::Directory
                                        && diff_entry.path.starts_with(&maybe_parent_folder.path)
                                    {
                                        match maybe_parent_folder.diff_type {
                                            DiffType::Removed => true,
                                            _ => false,
                                        }
                                    } else {
                                        false
                                    }
                                }
                                None => false,
                            },
                            None => false,
                        };
                        if parent_folder_removed {
                            diff_entries.remove(i);
                        }
                    }
                    _ => {}
                },
            },
            None => break,
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::diff_entry::{DiffEntry, DiffType, FileType};

    use super::optimize_diff_entries;

    #[test]
    fn remove_modified_folder() {
        let folder_diff_entry = DiffEntry {
            path: "folder".into(),
            file_type: FileType::Directory,
            diff_type: DiffType::Modified(()),
        };
        let file_diff_entry = DiffEntry {
            path: "folder/file".into(),
            file_type: FileType::RegularFile,
            diff_type: DiffType::Created(()),
        };
        let mut diff_entries = [folder_diff_entry.clone(), file_diff_entry.clone()].to_vec();
        optimize_diff_entries(&mut diff_entries);
        assert_eq!(diff_entries, vec![file_diff_entry])
    }

    #[test]
    fn remove_created_folder() {
        let folder_diff_entry = DiffEntry {
            path: "folder".into(),
            file_type: FileType::Directory,
            diff_type: DiffType::Created(()),
        };
        let file_diff_entry = DiffEntry {
            path: "folder/file".into(),
            file_type: FileType::RegularFile,
            diff_type: DiffType::Created(()),
        };
        let mut diff_entries = [folder_diff_entry.clone(), file_diff_entry.clone()].to_vec();
        optimize_diff_entries(&mut diff_entries);
        assert_eq!(diff_entries, vec![file_diff_entry])
    }

    #[test]
    fn preserve_empty_created_folders() {
        let folder_diff_entry = DiffEntry {
            path: "folder".into(),
            file_type: FileType::Directory,
            diff_type: DiffType::Created(()),
        };
        let mut diff_entries = [folder_diff_entry.clone()].to_vec();
        optimize_diff_entries(&mut diff_entries);
        assert_eq!(diff_entries, vec![folder_diff_entry])
    }

    #[test]
    fn remove_deleted_files_in_folder() {
        let folder_diff_entry = DiffEntry {
            path: "folder".into(),
            file_type: FileType::Directory,
            diff_type: DiffType::Removed,
        };
        let file_diff_entry = DiffEntry {
            path: "folder/file".into(),
            file_type: FileType::RegularFile,
            diff_type: DiffType::Removed,
        };
        let mut diff_entries = [folder_diff_entry.clone(), file_diff_entry.clone()].to_vec();
        optimize_diff_entries::<()>(&mut diff_entries);
        assert_eq!(diff_entries, vec![folder_diff_entry])
    }

    #[test]
    fn removes_files_when_folder_is_not_removed() {
        let file_diff_entry = DiffEntry {
            path: "folder/file".into(),
            file_type: FileType::RegularFile,
            diff_type: DiffType::Removed,
        };
        let mut diff_entries = [file_diff_entry.clone()].to_vec();
        optimize_diff_entries::<()>(&mut diff_entries);
        assert_eq!(diff_entries, vec![file_diff_entry])
    }

    #[test]
    fn removes_two_files() {
        let file_0_diff_entry = DiffEntry {
            path: "file".into(),
            file_type: FileType::RegularFile,
            diff_type: DiffType::Removed,
        };
        let file_1_diff_entry = DiffEntry {
            path: "file_more_name".into(),
            file_type: FileType::RegularFile,
            diff_type: DiffType::Removed,
        };
        let mut diff_entries = [file_0_diff_entry.clone(), file_1_diff_entry.clone()].to_vec();
        optimize_diff_entries::<()>(&mut diff_entries);
        assert_eq!(diff_entries, vec![file_0_diff_entry, file_1_diff_entry])
    }
}
