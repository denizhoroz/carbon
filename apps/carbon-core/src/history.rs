//! Undo for explorer file operations (create, rename, move, copy). Deletes go
//! to the Recycle Bin / Trash and are restored from there, not from here.

use std::path::PathBuf;

/// How many operations undo remembers.
pub const HISTORY_LIMIT: usize = 100;

/// A finished file operation, with what is needed to reverse it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileOp {
    Created(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
    Moved(Vec<(PathBuf, PathBuf)>),
    Copied(Vec<PathBuf>),
}

impl FileOp {
    /// Short description for "Undid ..." messages.
    pub fn label(&self) -> String {
        let name = |path: &PathBuf| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let count = |n: usize, one: &str| match n {
            1 => one.to_string(),
            n => format!("{n} items"),
        };
        match self {
            FileOp::Created(path) => format!("create '{}'", name(path)),
            FileOp::Renamed { from, to } => format!("rename '{}' to '{}'", name(from), name(to)),
            FileOp::Moved(moves) => {
                let one = moves.first().map(|(from, _)| format!("'{}'", name(from)));
                format!("move {}", count(moves.len(), &one.unwrap_or_default()))
            }
            FileOp::Copied(copies) => {
                let one = copies.first().map(|path| format!("'{}'", name(path)));
                format!("copy {}", count(copies.len(), &one.unwrap_or_default()))
            }
        }
    }
}
