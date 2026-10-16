//! Source control status. Only the current git branch for now, read straight
//! from `HEAD` (no git binary or library), so it is cheap to poll.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Branch {
    /// On a branch, e.g. `main`.
    Named(String),
    /// Detached `HEAD`; short (7 char) commit id.
    Detached(String),
}

impl Branch {
    /// Text for the status bar.
    pub fn label(&self) -> &str {
        match self {
            Branch::Named(name) | Branch::Detached(name) => name,
        }
    }
}

/// The branch of the git repository containing `path` (searching parent
/// folders), or `None` outside a repository.
pub fn current_branch(path: &Path) -> Option<Branch> {
    let git_dir = find_git_dir(path)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref:") {
        let reference = reference.trim();
        let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
        Some(Branch::Named(name.to_string()))
    } else if head.len() >= 7 && head.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(Branch::Detached(head[..7].to_string()))
    } else {
        None
    }
}

/// `.git` is a folder in normal clones and a file (`gitdir: <path>`) in worktrees
/// and submodules.
fn find_git_dir(path: &Path) -> Option<PathBuf> {
    for dir in path.ancestors() {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            let text = std::fs::read_to_string(&dot_git).ok()?;
            let target = text.trim().strip_prefix("gitdir:")?.trim();
            let target = Path::new(target);
            return Some(if target.is_absolute() {
                target.to_path_buf()
            } else {
                dir.join(target)
            });
        }
    }
    None
}
