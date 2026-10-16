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

/// Moves each of `paths` into folder `dir`, keeping names, and returns the
/// (old, new) pairs. Checks everything first: no folder into itself or below
/// it, no existing names, so a refusal changes nothing. Paths already in
/// `dir` are skipped.
pub fn move_into(paths: &[PathBuf], dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    if !dir.is_dir() {
        return Err(CoreError::NotADirectory(dir.to_path_buf()));
    }
    let mut moves = Vec::new();
    for path in paths {
        if path.parent() == Some(dir) {
            continue;
        }
        if dir.starts_with(path) {
            return Err(CoreError::IntoItself(path.clone()));
        }
        let name = path
            .file_name()
            .ok_or_else(|| CoreError::InvalidName(path.display().to_string()))?;
        let target = dir.join(name);
        if target.symlink_metadata().is_ok() {
            return Err(CoreError::AlreadyExists(target));
        }
        moves.push((path.clone(), target));
    }
    for (from, to) in &moves {
        std::fs::rename(from, to).map_err(|err| CoreError::io(from, err))?;
    }
    Ok(moves)
}

/// Copies each of `paths` (files or whole folders) into folder `dir` and
/// returns the new paths. A taken name gets " copy", then " copy 2", ...
/// before the extension, as in VS Code.
pub fn copy_into(paths: &[PathBuf], dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Err(CoreError::NotADirectory(dir.to_path_buf()));
    }
    if let Some(path) = paths
        .iter()
        .find(|path| path.is_dir() && dir.starts_with(path))
    {
        return Err(CoreError::IntoItself(path.clone()));
    }
    let mut copies = Vec::new();
    for path in paths {
        let target = free_name(dir, path)?;
        copy_recursive(path, &target)?;
        copies.push(target);
    }
    Ok(copies)
}

/// `dir/name`, or `dir/name copy[ N].ext` when that is taken.
fn free_name(dir: &Path, path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| CoreError::InvalidName(path.display().to_string()))?
        .to_string_lossy()
        .into_owned();
    let plain = dir.join(&name);
    if plain.symlink_metadata().is_err() {
        return Ok(plain);
    }
    // Folders and dotfiles keep the whole name as the stem.
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 && !path.is_dir() => (&name[..dot], &name[dot..]),
        _ => (name.as_str(), ""),
    };
    for n in 1.. {
        let suffix = if n == 1 {
            " copy".to_string()
        } else {
            format!(" copy {n}")
        };
        let candidate = dir.join(format!("{stem}{suffix}{ext}"));
        if candidate.symlink_metadata().is_err() {
            return Ok(candidate);
        }
    }
    unreachable!("an unused name exists")
}

/// Real folders are copied entry by entry; links are not followed into
/// folders (no loops), a link to a file copies the file's content.
fn copy_recursive(from: &Path, to: &Path) -> Result<()> {
    let meta = from
        .symlink_metadata()
        .map_err(|err| CoreError::io(from, err))?;
    if meta.is_dir() {
        std::fs::create_dir(to).map_err(|err| CoreError::io(to, err))?;
        for entry in std::fs::read_dir(from).map_err(|err| CoreError::io(from, err))? {
            let entry = entry.map_err(|err| CoreError::io(from, err))?;
            copy_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(from, to).map_err(|err| CoreError::io(from, err))?;
    }
    Ok(())
}
