//! Git status of workspace files for explorer markers, read with gitoxide
//! (FEATURE SC-06: gitoxide reads, git CLI writes). No git binary needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gix::diff::index::ChangeRef;
use gix::status::index_worktree::iter::Summary;

use crate::{CoreError, Result};

/// One file's state, ordered by how strongly it is shown (a folder shows the
/// strongest state among its files).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitStatus {
    /// Moved or renamed (staged or detected).
    Renamed,
    /// Not tracked yet.
    Untracked,
    /// New in the index (staged).
    Added,
    /// Content or type differs from the index or `HEAD`.
    Modified,
    /// Tracked, but gone from the worktree.
    Deleted,
    /// Unresolved merge conflict.
    Conflict,
}

impl GitStatus {
    /// One-letter badge, as VS Code shows it.
    pub fn letter(self) -> &'static str {
        match self {
            GitStatus::Renamed => "R",
            GitStatus::Untracked => "U",
            GitStatus::Added => "A",
            GitStatus::Modified => "M",
            GitStatus::Deleted => "D",
            GitStatus::Conflict => "!",
        }
    }
}

/// Status of changed files plus, for each folder holding one, the strongest
/// state inside it. Clean files are absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatusMap {
    files: HashMap<PathBuf, GitStatus>,
    dirs: HashMap<PathBuf, GitStatus>,
}

impl GitStatusMap {
    /// Builds the folder roll-up for `files`, up to (not including) `root`.
    pub fn new(root: &Path, files: HashMap<PathBuf, GitStatus>) -> Self {
        let mut dirs: HashMap<PathBuf, GitStatus> = HashMap::new();
        for (path, status) in &files {
            for dir in path.ancestors().skip(1) {
                if !dir.starts_with(root) || dir == root {
                    break;
                }
                let slot = dirs.entry(dir.to_path_buf()).or_insert(*status);
                *slot = (*slot).max(*status);
            }
        }
        Self { files, dirs }
    }

    /// A file's own state.
    pub fn file(&self, path: &Path) -> Option<GitStatus> {
        self.files.get(path).copied()
    }

    /// The strongest state of anything inside a folder.
    pub fn dir(&self, path: &Path) -> Option<GitStatus> {
        self.dirs.get(path).copied()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Reads the status of the repository containing `root`, limited to files
/// under `root`. `Ok(None)` when `root` is not in a git repository. Takes a
/// while on big repositories: call it off the UI thread.
pub fn git_status(root: &Path) -> Result<Option<GitStatusMap>> {
    let Ok(repo) = gix::discover(root) else {
        return Ok(None);
    };
    let Some(workdir) = repo.workdir() else {
        return Ok(None); // bare repository
    };
    let workdir = dunce::canonicalize(workdir).map_err(|err| CoreError::io(workdir, err))?;
    let items = repo
        .status(gix::progress::Discard)
        .map_err(git_error)?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(None)
        .map_err(git_error)?;

    let mut files: HashMap<PathBuf, GitStatus> = HashMap::new();
    for item in items {
        let item = item.map_err(git_error)?;
        let status = match &item {
            gix::status::Item::IndexWorktree(change) => match change.summary() {
                Some(Summary::Added) => GitStatus::Untracked,
                Some(Summary::Removed) => GitStatus::Deleted,
                Some(Summary::Modified | Summary::TypeChange) => GitStatus::Modified,
                Some(Summary::Renamed | Summary::Copied) => GitStatus::Renamed,
                Some(Summary::IntentToAdd) => GitStatus::Added,
                Some(Summary::Conflict) => GitStatus::Conflict,
                None => continue,
            },
            gix::status::Item::TreeIndex(change) => match change {
                ChangeRef::Addition { .. } => GitStatus::Added,
                ChangeRef::Deletion { .. } => GitStatus::Deleted,
                ChangeRef::Modification { .. } => GitStatus::Modified,
                ChangeRef::Rewrite { .. } => GitStatus::Renamed,
            },
        };
        // Repository paths use `/`; push parts so Windows paths match the tree's.
        let mut path = workdir.clone();
        for part in item.location().to_string().split('/') {
            path.push(part);
        }
        if !path.starts_with(root) {
            continue;
        }
        let slot = files.entry(path).or_insert(status);
        *slot = (*slot).max(status);
    }
    Ok(Some(GitStatusMap::new(root, files)))
}

fn git_error(err: impl std::fmt::Display) -> CoreError {
    CoreError::Git(err.to_string())
}
