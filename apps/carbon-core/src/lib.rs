//! Carbon core: the state the UI panels show, as plain Rust with no UI dependency.
//!
//! In-process for now (PROJECT.md, option a); the API is shaped so it can move
//! behind a daemon later. Long-running work (terminal output) is reported through
//! `async_channel` receivers, which work from both threads and async executors.

mod agent;
mod buffer;
mod error;
mod file_ops;
mod file_tree;
mod id;
mod jobs;
mod terminal;
mod vcs;
mod watcher;
mod workspace;

use std::path::{Path, PathBuf};

pub use agent::{AgentSession, AgentSessionId, AgentStatus, AgentStore, Message, Role};
pub use buffer::{Buffer, BufferId, BufferStore, LineEnding};
pub use error::{CoreError, Result};
pub use file_ops::{create_file, create_folder, move_to_trash, rename, validate_name};
pub use file_tree::{EntryKind, FileEntry, FileTree, Row, list_dir};
pub use jobs::{Job, JobId, JobStatus, JobStore};
pub use ropey::Rope;
pub use terminal::{Terminal, TerminalEvent, TerminalId, TerminalSpec, TerminalStore};
pub use vcs::{Branch, current_branch};
pub use watcher::{FsChanges, FsWatcher};
pub use workspace::Workspace;

/// Everything one window works on: the open folder and its sessions.
#[derive(Default)]
pub struct Core {
    workspace: Option<Workspace>,
    /// Explorer tree of the workspace; reset with it.
    tree: Option<FileTree>,
    watcher: Option<FsWatcher>,
    watch_error: Option<String>,
    /// The buffer the user opened last (the editor shows it).
    active_buffer: Option<BufferId>,
    pub buffers: BufferStore,
    pub terminals: TerminalStore,
    pub agents: AgentStore,
    pub jobs: JobStore,
}

impl Core {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a folder as the workspace, replacing any open one.
    pub fn open_workspace(&mut self, path: impl AsRef<Path>) -> Result<&Workspace> {
        let workspace = Workspace::open(path)?;
        self.tree = Some(FileTree::new(workspace.root())?);
        // Without a watcher the tree still works; it just does not update live.
        (self.watcher, self.watch_error) = match FsWatcher::new(workspace.root()) {
            Ok(watcher) => (Some(watcher), None),
            Err(err) => (None, Some(err.to_string())),
        };
        Ok(self.workspace.insert(workspace))
    }

    /// Creates folder `name` in `parent` and opens it as the workspace.
    pub fn create_workspace(&mut self, parent: impl AsRef<Path>, name: &str) -> Result<&Workspace> {
        let path = create_folder(parent.as_ref(), name)?;
        self.open_workspace(path)
    }

    /// Closes the workspace. Buffers, terminals and agent sessions stay open.
    pub fn close_workspace(&mut self) {
        self.workspace = None;
        self.tree = None;
        self.watcher = None;
        self.watch_error = None;
    }

    pub fn workspace(&self) -> Option<&Workspace> {
        self.workspace.as_ref()
    }

    /// Git branch of the workspace, read fresh from disk (cheap; safe to poll).
    pub fn branch(&self) -> Option<Branch> {
        current_branch(self.workspace.as_ref()?.root())
    }

    /// Lists one directory of the workspace for the explorer. `dir` may be
    /// absolute or relative to the workspace root, and must stay inside it.
    pub fn list_dir(&self, dir: impl AsRef<Path>) -> Result<Vec<FileEntry>> {
        let workspace = self.workspace.as_ref().ok_or(CoreError::NoWorkspace)?;
        let dir = workspace.resolve(dir)?;
        list_dir(&dir)
    }

    /// Explorer tree of the open workspace.
    pub fn tree(&self) -> Option<&FileTree> {
        self.tree.as_ref()
    }

    pub fn tree_mut(&mut self) -> Option<&mut FileTree> {
        self.tree.as_mut()
    }

    /// Debounced file system changes in the workspace; `None` without a
    /// workspace or when watching failed (see `watch_error`).
    pub fn fs_changes(&self) -> Option<async_channel::Receiver<FsChanges>> {
        self.watcher.as_ref().map(FsWatcher::changes)
    }

    /// Why the workspace is not watched, if watching failed.
    pub fn watch_error(&self) -> Option<&str> {
        self.watch_error.as_deref()
    }

    /// Re-lists the folders a change batch names; returns whether the tree changed.
    pub fn apply_fs_changes(&mut self, changes: &FsChanges) -> Result<bool> {
        let Some(tree) = self.tree.as_mut() else {
            return Ok(false);
        };
        if changes.rescan {
            return tree.refresh_all();
        }
        let mut dirs = changes.dirs.clone();
        // Parents first, so a vanished folder is dropped before its children.
        dirs.sort_by_key(|dir| dir.components().count());
        let mut changed = false;
        for dir in &dirs {
            changed |= tree.refresh(dir)?;
        }
        Ok(changed)
    }

    /// New empty file `name` in workspace folder `dir`; the folder opens in the
    /// tree so the file shows at once.
    pub fn create_file(&mut self, dir: impl AsRef<Path>, name: &str) -> Result<PathBuf> {
        let dir = self.resolve(dir)?;
        let path = create_file(&dir, name)?;
        self.show_in_tree(&dir)?;
        Ok(path)
    }

    /// New empty folder `name` in workspace folder `dir`.
    pub fn create_dir(&mut self, dir: impl AsRef<Path>, name: &str) -> Result<PathBuf> {
        let dir = self.resolve(dir)?;
        let path = create_folder(&dir, name)?;
        self.show_in_tree(&dir)?;
        Ok(path)
    }

    /// Renames a workspace entry in place; open buffers follow it.
    pub fn rename(&mut self, path: impl AsRef<Path>, new_name: &str) -> Result<PathBuf> {
        let path = self.resolve_entry(path)?;
        let new_path = rename(&path, new_name)?;
        self.buffers.moved(&path, &new_path);
        self.refresh_parent(&path)?;
        Ok(new_path)
    }

    /// Moves a workspace entry to the Recycle Bin / Trash. Open buffers stay
    /// open (unsaved work is not lost).
    pub fn trash(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = self.resolve_entry(path)?;
        move_to_trash(&path)?;
        self.refresh_parent(&path)
    }

    fn resolve(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        self.workspace
            .as_ref()
            .ok_or(CoreError::NoWorkspace)?
            .resolve(path)
    }

    /// Like `resolve`, but not the root, and without following a symlink
    /// (renaming or deleting a link acts on the link).
    fn resolve_entry(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let workspace = self.workspace.as_ref().ok_or(CoreError::NoWorkspace)?;
        let path = path.as_ref();
        if workspace
            .resolve(path)
            .is_ok_and(|full| full == workspace.root())
        {
            return Err(CoreError::WorkspaceRoot);
        }
        let parent = path.parent().ok_or(CoreError::WorkspaceRoot)?;
        let name = path.file_name().ok_or(CoreError::WorkspaceRoot)?;
        let entry = workspace.resolve(parent)?.join(name);
        if entry == workspace.root() || !entry.starts_with(workspace.root()) {
            return Err(CoreError::WorkspaceRoot);
        }
        Ok(entry)
    }

    /// Expands `dir` in the tree and lists it again.
    fn show_in_tree(&mut self, dir: &Path) -> Result<()> {
        if let Some(tree) = self.tree.as_mut() {
            tree.expand(dir)?;
            tree.refresh(dir)?;
        }
        Ok(())
    }

    fn refresh_parent(&mut self, path: &Path) -> Result<()> {
        if let (Some(tree), Some(parent)) = (self.tree.as_mut(), path.parent()) {
            tree.refresh(parent)?;
        }
        Ok(())
    }

    /// Opens a workspace file into a buffer (or returns the already open one)
    /// and makes it the active buffer.
    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<BufferId> {
        let workspace = self.workspace.as_ref().ok_or(CoreError::NoWorkspace)?;
        let path = workspace.resolve(path)?;
        let id = self.buffers.open(path)?;
        self.active_buffer = Some(id);
        Ok(id)
    }

    /// The buffer opened last, if it is still open.
    pub fn active_buffer(&self) -> Option<&Buffer> {
        self.buffers.get(self.active_buffer?)
    }

    /// Starts a terminal; without an explicit working directory it starts in
    /// the workspace root.
    pub fn spawn_terminal(&mut self, mut spec: TerminalSpec) -> Result<TerminalId> {
        if spec.cwd.is_none() {
            spec.cwd = self.workspace.as_ref().map(|w| w.root().to_path_buf());
        }
        self.terminals.spawn(spec)
    }
}
