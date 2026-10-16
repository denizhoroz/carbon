use std::path::{Component, Path, PathBuf};

use crate::{CoreError, Result};

/// An open folder. All workspace paths resolve inside `root`.
#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
    name: String,
}

impl Workspace {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        // `dunce` keeps Windows paths in normal `C:\...` form instead of `\\?\C:\...`.
        let root = dunce::canonicalize(path).map_err(|err| CoreError::io(path, err))?;
        if !root.is_dir() {
            return Err(CoreError::NotADirectory(root));
        }
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string());
        Ok(Self { root, name })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Folder name, shown as the workspace title.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Absolute path for `path` (absolute, or relative to the root); errors when
    /// it leaves the workspace, including through `..`. Works for paths that do
    /// not exist yet (new files).
    pub fn resolve(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let normalized = normalize(&joined);
        // Existing paths are canonicalized so symlinks and case cannot escape the root.
        let resolved = dunce::canonicalize(&normalized).unwrap_or(normalized);
        if resolved.starts_with(&self.root) {
            Ok(resolved)
        } else {
            Err(CoreError::OutsideWorkspace(path.to_path_buf()))
        }
    }

    /// `path` relative to the root, for display.
    pub fn relative<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        path.strip_prefix(&self.root).ok()
    }
}

/// Removes `.` and resolves `..` lexically (no file system access).
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}
