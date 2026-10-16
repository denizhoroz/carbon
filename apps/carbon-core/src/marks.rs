//! Per-file markers the explorer shows next to names: problem counts (fed by
//! diagnostics later) and "an agent edited this" (fed by agent sessions later).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Markers of one file, or the sum over a folder's files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mark {
    pub errors: u32,
    pub warnings: u32,
    pub agent_edited: bool,
}

impl Mark {
    pub fn is_empty(&self) -> bool {
        *self == Mark::default()
    }

    fn add(&mut self, other: Mark) {
        self.errors += other.errors;
        self.warnings += other.warnings;
        self.agent_edited |= other.agent_edited;
    }
}

/// File markers plus each folder's roll-up (kept current on every change).
#[derive(Clone, Debug, Default)]
pub struct Marks {
    files: HashMap<PathBuf, Mark>,
    dirs: HashMap<PathBuf, Mark>,
}

impl Marks {
    /// Problem counts of a file (from diagnostics); zero clears them.
    pub fn set_problems(&mut self, path: &Path, errors: u32, warnings: u32) {
        let mut mark = self.file(path);
        (mark.errors, mark.warnings) = (errors, warnings);
        self.set(path, mark);
    }

    /// Records that an agent edited (or stopped being the last editor of) a file.
    pub fn set_agent_edited(&mut self, path: &Path, edited: bool) {
        let mut mark = self.file(path);
        mark.agent_edited = edited;
        self.set(path, mark);
    }

    pub fn clear(&mut self) {
        self.files.clear();
        self.dirs.clear();
    }

    pub fn file(&self, path: &Path) -> Mark {
        self.files.get(path).copied().unwrap_or_default()
    }

    /// Sum over everything inside a folder.
    pub fn dir(&self, path: &Path) -> Mark {
        self.dirs.get(path).copied().unwrap_or_default()
    }

    /// Follows a rename or move (of a file or a whole folder).
    pub fn moved(&mut self, old: &Path, new: &Path) {
        let moved: Vec<(PathBuf, Mark)> = self
            .files
            .iter()
            .filter_map(|(path, mark)| {
                let rest = path.strip_prefix(old).ok()?;
                Some((join(new, rest), *mark))
            })
            .collect();
        self.files.retain(|path, _| !path.starts_with(old));
        self.files.extend(moved);
        self.rebuild_dirs();
    }

    fn set(&mut self, path: &Path, mark: Mark) {
        if mark.is_empty() {
            self.files.remove(path);
        } else {
            self.files.insert(path.to_path_buf(), mark);
        }
        self.rebuild_dirs();
    }

    /// Recomputed from scratch: markers change rarely and are few.
    fn rebuild_dirs(&mut self) {
        self.dirs.clear();
        for (path, mark) in &self.files {
            for dir in path.ancestors().skip(1) {
                self.dirs.entry(dir.to_path_buf()).or_default().add(*mark);
            }
        }
    }
}

fn join(base: &Path, rest: &Path) -> PathBuf {
    if rest.as_os_str().is_empty() {
        base.to_path_buf()
    } else {
        base.join(rest)
    }
}
