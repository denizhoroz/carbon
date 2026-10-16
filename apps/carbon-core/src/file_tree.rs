use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{CoreError, Result};

/// Never listed, like VS Code's default `files.exclude`.
const ALWAYS_EXCLUDED: &[&str] = &[".git", ".hg", ".svn", "CVS", ".DS_Store", "Thumbs.db"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    File,
    Symlink,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    /// Matched by a `.gitignore` / `.ignore` rule; the explorer shows it dimmed.
    pub ignored: bool,
}

/// Lists one directory (not recursive; the explorer loads folders as they open).
/// Folders first, then case-insensitive by name. Ignore rules from parent
/// folders apply, with or without a git repository.
pub fn list_dir(dir: &Path) -> Result<Vec<FileEntry>> {
    if !dir.is_dir() {
        return Err(CoreError::NotADirectory(dir.to_path_buf()));
    }
    let not_ignored: HashSet<PathBuf> = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .require_git(false)
        .build()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into_path())
        .collect();

    let read = std::fs::read_dir(dir).map_err(|err| CoreError::io(dir, err))?;
    let mut entries = Vec::new();
    for entry in read {
        let entry = entry.map_err(|err| CoreError::io(dir, err))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if ALWAYS_EXCLUDED.contains(&name.as_str()) {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|err| CoreError::io(entry.path(), err))?;
        let kind = if file_type.is_symlink() {
            EntryKind::Symlink
        } else if file_type.is_dir() {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        let path = entry.path();
        let ignored = !not_ignored.contains(&path);
        entries.push(FileEntry {
            name,
            path,
            kind,
            ignored,
        });
    }
    entries.sort_by(|a, b| {
        (a.kind != EntryKind::Dir)
            .cmp(&(b.kind != EntryKind::Dir))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}
