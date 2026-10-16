use std::collections::{HashMap, HashSet};
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

/// One visible line of the explorer: an entry and how deep it is (root children = 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The entry the row stands for (for a compact chain, the deepest folder).
    pub entry: FileEntry,
    pub depth: usize,
    /// Shown name: the entry's name, or `a/b/c` for a compact folder chain.
    pub label: String,
}

/// The explorer's tree: folders load when they expand (lazy), and the visible
/// rows are a flat list (what a virtualized list renders).
///
/// Loaded folders are the root plus every folder in `expanded`. Collapsing a
/// folder keeps its expanded descendants, so they open again with it.
///
/// Compact folders (on by default, like VS Code): an open folder whose only
/// entry is a folder shares one row with it (`src/main/java`), and opening a
/// folder opens such a chain all the way down.
#[derive(Debug)]
pub struct FileTree {
    root: PathBuf,
    expanded: HashSet<PathBuf>,
    children: HashMap<PathBuf, Vec<FileEntry>>,
    compact: bool,
}

impl FileTree {
    /// Loads the root listing. `root` should be canonical (see `Workspace::open`).
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let listing = list_dir(&root)?;
        let mut children = HashMap::new();
        children.insert(root.clone(), listing);
        Ok(Self {
            root,
            expanded: HashSet::new(),
            children,
            compact: true,
        })
    }

    pub fn set_compact(&mut self, compact: bool) {
        self.compact = compact;
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn is_expanded(&self, dir: &Path) -> bool {
        self.expanded.contains(dir)
    }

    /// Whether `dir` is listed (the root or an expanded folder); only these
    /// need refreshing when the file system changes.
    pub fn is_loaded(&self, dir: &Path) -> bool {
        self.children.contains_key(dir)
    }

    /// Opens a folder, listing it on first open.
    pub fn expand(&mut self, dir: &Path) -> Result<()> {
        if !dir.starts_with(&self.root) {
            return Err(CoreError::OutsideWorkspace(dir.to_path_buf()));
        }
        if dir == self.root {
            return Ok(());
        }
        if !self.children.contains_key(dir) {
            let listing = self.list(dir)?;
            self.children.insert(dir.to_path_buf(), listing);
        }
        self.expanded.insert(dir.to_path_buf());
        if self.compact
            && let Some(only) = self.only_subfolder(dir)
        {
            return self.expand(&only);
        }
        Ok(())
    }

    /// The single entry of a loaded folder, if that entry is a folder.
    fn only_subfolder(&self, dir: &Path) -> Option<PathBuf> {
        match self.children.get(dir)?.as_slice() {
            [only] if only.kind == EntryKind::Dir => Some(only.path.clone()),
            _ => None,
        }
    }

    pub fn collapse(&mut self, dir: &Path) {
        if self.expanded.remove(dir) {
            self.children.remove(dir);
        }
    }

    /// Expands a collapsed folder or collapses an expanded one.
    pub fn toggle(&mut self, dir: &Path) -> Result<()> {
        if self.is_expanded(dir) {
            self.collapse(dir);
            Ok(())
        } else {
            self.expand(dir)
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        let root = self.root.clone();
        self.children.retain(|dir, _| *dir == root);
    }

    /// Lists `dir` again if it is loaded; returns whether anything changed.
    /// A folder that no longer exists is dropped with its descendants.
    pub fn refresh(&mut self, dir: &Path) -> Result<bool> {
        if !self.children.contains_key(dir) {
            return Ok(false);
        }
        let listing = match self.list(dir) {
            Ok(listing) => listing,
            Err(CoreError::NotADirectory(_)) if dir != self.root => {
                self.forget(dir);
                return Ok(true);
            }
            Err(err) => return Err(err),
        };
        let changed = self.children.get(dir) != Some(&listing);
        if changed {
            // Expanded folders that are gone from this listing go too.
            let gone: Vec<PathBuf> = self.children[dir]
                .iter()
                .filter(|old| old.kind == EntryKind::Dir)
                .filter(|old| !listing.iter().any(|new| new.path == old.path))
                .map(|old| old.path.clone())
                .collect();
            for path in gone {
                self.forget(&path);
            }
            self.children.insert(dir.to_path_buf(), listing);
        }
        Ok(changed)
    }

    /// Lists every loaded folder again (after the watcher lost events).
    pub fn refresh_all(&mut self) -> Result<bool> {
        let mut dirs: Vec<PathBuf> = self.children.keys().cloned().collect();
        // Parents first, so a vanished folder is dropped before its children.
        dirs.sort_by_key(|dir| dir.components().count());
        let mut changed = false;
        for dir in dirs {
            changed |= self.refresh(&dir)?;
        }
        Ok(changed)
    }

    /// Visible rows, depth first: each expanded folder's children follow it.
    pub fn visible_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.push_rows(&self.root, 0, &mut rows);
        rows
    }

    fn push_rows(&self, dir: &Path, depth: usize, rows: &mut Vec<Row>) {
        let Some(entries) = self.children.get(dir) else {
            return;
        };
        for entry in entries {
            let mut entry = entry;
            let mut label = entry.name.clone();
            // Compact chain: while this folder is open and holds only a folder,
            // that folder joins the row.
            while self.compact
                && entry.kind == EntryKind::Dir
                && self.expanded.contains(&entry.path)
                && let Some([only]) = self.children.get(&entry.path).map(Vec::as_slice)
                && only.kind == EntryKind::Dir
            {
                label = format!("{label}/{}", only.name);
                entry = only;
            }
            rows.push(Row {
                entry: entry.clone(),
                depth,
                label,
            });
            if entry.kind == EntryKind::Dir && self.expanded.contains(&entry.path) {
                self.push_rows(&entry.path, depth + 1, rows);
            }
        }
    }

    /// Lists `dir`. Everything inside an ignored folder is ignored too: git does
    /// not look inside it, but ignore rules only match the folder itself.
    fn list(&self, dir: &Path) -> Result<Vec<FileEntry>> {
        let mut listing = list_dir(dir)?;
        if self.is_ignored(dir) {
            for entry in &mut listing {
                entry.ignored = true;
            }
        }
        Ok(listing)
    }

    /// Whether `path` is listed (in its loaded parent) as ignored.
    fn is_ignored(&self, path: &Path) -> bool {
        path.parent()
            .and_then(|parent| self.children.get(parent))
            .and_then(|entries| entries.iter().find(|entry| entry.path == path))
            .is_some_and(|entry| entry.ignored)
    }

    /// Drops `dir` and everything under it from the expanded set and cache.
    fn forget(&mut self, dir: &Path) {
        self.expanded.retain(|path| !path.starts_with(dir));
        self.children.retain(|path, _| !path.starts_with(dir));
    }
}
