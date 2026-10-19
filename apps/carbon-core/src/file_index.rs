//! Every file of a workspace, for Quick Open (go to file).

use std::path::Path;

use crate::file_tree::ALWAYS_EXCLUDED;

/// Most files `list_files` returns by default; beyond it the list is cut.
pub const FILE_INDEX_LIMIT: usize = 100_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileList {
    /// Paths relative to the root, `/`-separated, sorted.
    pub files: Vec<String>,
    /// More files exist than `limit` allowed.
    pub truncated: bool,
}

/// Lists the files under `root` recursively, with the same rules as the
/// explorer: `.gitignore` / `.ignore` apply with or without git, hidden files
/// are included, and `ALWAYS_EXCLUDED` names are skipped. Symlinked folders
/// are not followed. Blocking: run it off the UI thread.
pub fn list_files(root: &Path, limit: usize) -> FileList {
    let walk = ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|entry| {
            !ALWAYS_EXCLUDED.contains(&entry.file_name().to_string_lossy().as_ref())
        })
        .build();
    let mut list = FileList::default();
    for entry in walk.filter_map(|entry| entry.ok()) {
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        if list.files.len() == limit {
            list.truncated = true;
            break;
        }
        let parts: Vec<_> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect();
        list.files.push(parts.join("/"));
    }
    list.files.sort();
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src/nested")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        for file in [
            "src/main.rs",
            "src/nested/deep.rs",
            "target/debug/out.bin",
            ".git/HEAD",
            ".env",
            ".gitignore",
            "README.md",
        ] {
            std::fs::write(root.join(file), "").unwrap();
        }
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        dir
    }

    #[test]
    fn honours_ignore_rules_without_git() {
        let dir = tree();
        let list = list_files(dir.path(), FILE_INDEX_LIMIT);
        assert_eq!(
            list.files,
            vec![
                ".env",
                ".gitignore",
                "README.md",
                "src/main.rs",
                "src/nested/deep.rs"
            ]
        );
        assert!(!list.truncated);
    }

    #[test]
    fn limit_truncates() {
        let dir = tree();
        let list = list_files(dir.path(), 2);
        assert_eq!(list.files.len(), 2);
        assert!(list.truncated);
    }
}
