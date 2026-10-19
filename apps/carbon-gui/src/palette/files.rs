//! Quick Open's file list: every file of the workspace, listed off the UI
//! thread and kept until the core says the files changed.

use std::path::PathBuf;
use std::sync::Arc;

use carbon_core::{Core, FILE_INDEX_LIMIT, fuzzy, list_files};
use gpui_kit::*;

/// The workspace's files, owned by the app so they outlive the palette.
pub struct FileIndex {
    /// Workspace root and core files generation the list was made for.
    listed_for: Option<(PathBuf, u64)>,
    pub files: Arc<Vec<String>>,
    pub truncated: bool,
    pub scanning: bool,
    _scan: Task<()>,
}

impl FileIndex {
    pub fn new() -> Self {
        Self {
            listed_for: None,
            files: Arc::default(),
            truncated: false,
            scanning: false,
            _scan: Task::ready(()),
        }
    }

    /// Re-lists the files when the workspace or its files changed since the
    /// last listing. The old list stays visible meanwhile; observers are
    /// notified when the new one is in.
    pub fn refresh(&mut self, core: &Entity<Core>, cx: &mut Context<Self>) {
        let core = core.read(cx);
        let Some(root) = core
            .workspace()
            .map(|workspace| workspace.root().to_path_buf())
        else {
            if self.listed_for.take().is_some() || !self.files.is_empty() {
                self.files = Arc::default();
                self.truncated = false;
                self.scanning = false;
                self._scan = Task::ready(());
                cx.notify();
            }
            return;
        };
        let key = (root.clone(), core.files_generation());
        if self.listed_for.as_ref() == Some(&key) {
            return;
        }
        if self
            .listed_for
            .as_ref()
            .is_some_and(|(old, _)| *old != root)
        {
            self.files = Arc::default();
        }
        self.listed_for = Some(key);
        self.scanning = true;
        let listing = cx.background_spawn(async move { list_files(&root, FILE_INDEX_LIMIT) });
        self._scan = cx.spawn(async move |this, cx| {
            let list = listing.await;
            _ = this.update(cx, |this, cx| {
                this.files = Arc::new(list.files);
                this.truncated = list.truncated;
                this.scanning = false;
                cx.notify();
            });
        });
        cx.notify();
    }
}

/// One file that matched a Quick Open query.
pub struct FileMatch {
    /// Relative, `/`-separated path.
    pub path: String,
    /// Matched char positions in `path`.
    pub positions: Vec<u32>,
}

/// Matches `query` against the files: best score first, then the shorter
/// path, then by name; at most `limit`. Blocking: run it off the UI thread.
pub fn match_files(query: &str, files: &[String], limit: usize) -> Vec<FileMatch> {
    let mut found = fuzzy::match_list(query, files.iter().map(String::as_str), true);
    found.sort_by(|a, b| {
        let (pa, pb) = (&files[a.index], &files[b.index]);
        b.score
            .cmp(&a.score)
            .then(pa.len().cmp(&pb.len()))
            .then(pa.cmp(pb))
    });
    found.truncate(limit);
    found
        .into_iter()
        .map(|found| FileMatch {
            path: files[found.index].clone(),
            positions: found.positions,
        })
        .collect()
}

/// Part of a path and its matched char positions.
pub type PathPart<'a> = (&'a str, Vec<u32>);

/// Splits a matched path into (folder, file name) and their matched char
/// positions, each relative to its own part. The folder is `""` at the root.
pub fn split_path<'a>(path: &'a str, positions: &[u32]) -> (PathPart<'a>, PathPart<'a>) {
    let Some(slash) = path.rfind('/') else {
        return (("", Vec::new()), (path, positions.to_vec()));
    };
    let (dir, name) = (&path[..slash], &path[slash + 1..]);
    let name_start = dir.chars().count() as u32 + 1;
    let dir_positions = positions
        .iter()
        .copied()
        .filter(|&p| p < name_start - 1)
        .collect();
    let name_positions = positions
        .iter()
        .copied()
        .filter(|&p| p >= name_start)
        .map(|p| p - name_start)
        .collect();
    ((dir, dir_positions), (name, name_positions))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: the gpui glob brings a `test` macro that shadows the built-in one.
    use super::{match_files, split_path};

    #[test]
    fn best_match_first_then_shorter() {
        let files: Vec<String> = [
            "docs/main.md",
            "app/src/bin/main.rs",
            "src/main.rs",
            "src/lib.rs",
        ]
        .map(String::from)
        .to_vec();
        let found = match_files("main.rs", &files, 10);
        let paths: Vec<&str> = found.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths[0], "src/main.rs");
        assert!(paths.contains(&"app/src/bin/main.rs"));
        assert!(!paths.contains(&"src/lib.rs"));
        assert_eq!(match_files("main", &files, 1).len(), 1);
    }

    #[test]
    fn splits_positions_by_part() {
        // s0 r1 c2 /3 m4 a5 i6 n7
        let ((dir, dir_pos), (name, name_pos)) = split_path("src/main", &[0, 4, 7]);
        assert_eq!((dir, name), ("src", "main"));
        assert_eq!(dir_pos, vec![0]);
        assert_eq!(name_pos, vec![0, 3]);
        let ((dir, _), (name, name_pos)) = split_path("README.md", &[0]);
        assert_eq!((dir, name), ("", "README.md"));
        assert_eq!(name_pos, vec![0]);
    }
}
