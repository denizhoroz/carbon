//! File operations behind the explorer: create, rename, move to trash.
//! Names are single plain entries; nothing here overwrites an existing path.

use std::path::{Path, PathBuf};

use crate::{CoreError, Result};

/// Checks a new entry name: not empty, not `.`/`..`, no trailing dot, no path
/// separators or characters Windows forbids. Returns it trimmed.
pub fn validate_name(name: &str) -> Result<&str> {
    let name = name.trim();
    const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || name
            .chars()
            .any(|c| FORBIDDEN.contains(&c) || c.is_control())
    {
        return Err(CoreError::InvalidName(name.to_string()));
    }
    Ok(name)
}

/// Path for a new entry `name` in `parent`; refuses a bad name, a missing
/// parent and an existing path.
fn new_entry_path(parent: &Path, name: &str) -> Result<PathBuf> {
    let name = validate_name(name)?;
    if !parent.is_dir() {
        return Err(CoreError::NotADirectory(parent.to_path_buf()));
    }
    let path = parent.join(name);
    if path.symlink_metadata().is_ok() {
        return Err(CoreError::AlreadyExists(path));
    }
    Ok(path)
}

/// Creates a new, empty folder `name` inside `parent` and returns its path.
pub fn create_folder(parent: &Path, name: &str) -> Result<PathBuf> {
    let path = new_entry_path(parent, name)?;
    std::fs::create_dir(&path).map_err(|err| CoreError::io(&path, err))?;
    Ok(path)
}

/// Creates a new, empty file `name` inside `parent` and returns its path.
pub fn create_file(parent: &Path, name: &str) -> Result<PathBuf> {
    let path = new_entry_path(parent, name)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|err| CoreError::io(&path, err))?;
    Ok(path)
}

/// Renames `path` to `new_name` in the same folder and returns the new path.
/// A change of letter case only is allowed (on Windows the old and new path
/// are the same file).
pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf> {
    let new_name = validate_name(new_name)?;
    let parent = path
        .parent()
        .ok_or_else(|| CoreError::InvalidName(path.display().to_string()))?;
    let new_path = parent.join(new_name);
    if new_path == path {
        return Ok(new_path);
    }
    if new_path.symlink_metadata().is_ok() && !same_entry(path, &new_path) {
        return Err(CoreError::AlreadyExists(new_path));
    }
    std::fs::rename(path, &new_path).map_err(|err| CoreError::io(path, err))?;
    Ok(new_path)
}

/// Moves `path` to the Recycle Bin / Trash (restorable), not a hard delete.
pub fn move_to_trash(path: &Path) -> Result<()> {
    trash::delete(path).map_err(|err| CoreError::Trash(format!("{}: {err}", path.display())))
}

/// Whether two paths name the same entry (case-insensitive file systems).
fn same_entry(a: &Path, b: &Path) -> bool {
    match (dunce::canonicalize(a), dunce::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
