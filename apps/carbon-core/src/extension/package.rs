//! Files inside an extension package. A package
//! arranges its folder as it likes; the manifest names each file by a path
//! relative to the package root. The rules keep a package working on every
//! OS and keep a path from leaving its package.

use std::path::{Path, PathBuf};

/// Longest path a manifest may name, in bytes (Windows' 260-character limit,
/// with room for the install folders above the package).
pub const MAX_PACKAGE_PATH: usize = 160;

/// Names Windows reserves for devices, with or without an extension.
const RESERVED_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Characters Windows does not allow in file names (besides control characters).
const BAD_CHARS: [char; 8] = ['<', '>', ':', '"', '|', '?', '*', '\\'];

/// Checks a package path as written in a manifest: relative, `/`-separated,
/// no `.` or `..`, no names or characters Windows rejects, not too long.
pub fn check_package_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path is empty".into());
    }
    if path.len() > MAX_PACKAGE_PATH {
        return Err(format!(
            "path `{path}` is longer than {MAX_PACKAGE_PATH} bytes"
        ));
    }
    if path.starts_with('/') {
        return Err(format!("path `{path}` must be relative to the package"));
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!(
                "path `{path}`: use plain names separated by `/` (no `.`, `..` or `//`)"
            ));
        }
        if let Some(bad) = part
            .chars()
            .find(|c| BAD_CHARS.contains(c) || c.is_control())
        {
            return Err(format!("path `{path}`: `{bad}` is not allowed in a name"));
        }
        if part.ends_with('.') || part.ends_with(' ') {
            return Err(format!(
                "path `{path}`: a name may not end with `.` or a space"
            ));
        }
        let stem = part.split('.').next().unwrap_or(part).to_ascii_lowercase();
        if RESERVED_NAMES.contains(&stem.as_str()) {
            return Err(format!(
                "path `{path}`: `{part}` is a reserved name on Windows"
            ));
        }
    }
    Ok(())
}

/// The file `path` names inside the package at `root`. Besides
/// [`check_package_path`]: every name must match the file on disk exactly,
/// case included (a package must also work where names are case-sensitive),
/// no step may be a symbolic link, and the target must be a file.
pub fn resolve_package_file(root: &Path, path: &str) -> Result<PathBuf, String> {
    check_package_path(path)?;
    let mut current = root.to_path_buf();
    let parts: Vec<&str> = path.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        let entries =
            std::fs::read_dir(&current).map_err(|err| format!("{}: {err}", current.display()))?;
        let entry = entries
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_str() == Some(part))
            .ok_or_else(|| format!("`{path}` is not in the package (names are case-sensitive)"))?;
        let kind = entry
            .file_type()
            .map_err(|err| format!("{}: {err}", entry.path().display()))?;
        if kind.is_symlink() {
            return Err(format!("`{path}`: links are not followed"));
        }
        let last = index + 1 == parts.len();
        if last && !kind.is_file() {
            return Err(format!("`{path}` is not a file"));
        }
        if !last && !kind.is_dir() {
            return Err(format!("`{path}`: `{part}` is not a folder"));
        }
        current = entry.path();
    }
    Ok(current)
}
