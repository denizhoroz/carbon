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
mod git_status;
mod history;
mod id;
mod jobs;
mod marks;
mod terminal;
mod vcs;
mod watcher;
mod workspace;

use std::path::{Path, PathBuf};

pub use agent::{AgentSession, AgentSessionId, AgentStatus, AgentStore, Message, Role};
pub use buffer::{Buffer, BufferId, BufferStore, LineEnding};
pub use error::{CoreError, Result};
pub use file_ops::{
    copy_into, create_file, create_folder, move_into, move_to_trash, rename, validate_name,
};
pub use file_tree::{EntryKind, FileEntry, FileTree, Row, list_dir};
pub use git_status::{GitStatus, GitStatusMap, git_status};
pub use history::{FileOp, HISTORY_LIMIT};
pub use jobs::{Job, JobId, JobStatus, JobStore};
pub use marks::{Mark, Marks};
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
    /// Git state of changed files (explorer colors and letters); set from a
    /// background `git_status` run.
    git: Option<GitStatusMap>,
    /// Problem and agent-edit markers (explorer badges).
    pub marks: Marks,
    /// Undo stack of file operations, newest last.
    history: Vec<FileOp>,
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
        self.git = None;
        self.marks.clear();
        self.history.clear();
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
        self.git = None;
        self.marks.clear();
        self.history.clear();
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
        self.record(FileOp::Created(path.clone()));
        self.show_in_tree(&dir)?;
        Ok(path)
    }

    /// New empty folder `name` in workspace folder `dir`.
    pub fn create_dir(&mut self, dir: impl AsRef<Path>, name: &str) -> Result<PathBuf> {
        let dir = self.resolve(dir)?;
        let path = create_folder(&dir, name)?;
        self.record(FileOp::Created(path.clone()));
        self.show_in_tree(&dir)?;
        Ok(path)
    }

    /// Renames a workspace entry in place; open buffers follow it.
    pub fn rename(&mut self, path: impl AsRef<Path>, new_name: &str) -> Result<PathBuf> {
        let path = self.resolve_entry(path)?;
        let new_path = rename(&path, new_name)?;
        self.moved(&path, &new_path);
        if new_path != path {
            self.record(FileOp::Renamed {
                from: path.clone(),
                to: new_path.clone(),
            });
        }
        self.refresh_parent(&path)?;
        Ok(new_path)
    }

    /// Moves entries into workspace folder `dir` (drag and drop, cut + paste);
    /// returns the new paths. Open buffers and markers follow.
    pub fn move_entries(
        &mut self,
        paths: &[PathBuf],
        dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>> {
        let dir = self.resolve(dir)?;
        let paths = paths
            .iter()
            .map(|path| self.resolve_entry(path))
            .collect::<Result<Vec<_>>>()?;
        let moves = move_into(&paths, &dir)?;
        for (from, to) in &moves {
            self.moved(from, to);
            self.refresh_parent(from)?;
        }
        self.show_in_tree(&dir)?;
        let new_paths = moves.iter().map(|(_, to)| to.clone()).collect();
        if !moves.is_empty() {
            self.record(FileOp::Moved(moves));
        }
        Ok(new_paths)
    }

    /// Copies entries (from the workspace, or from outside it: files dropped
    /// from the OS) into workspace folder `dir`; returns the new paths.
    pub fn copy_entries(
        &mut self,
        paths: &[PathBuf],
        dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>> {
        let dir = self.resolve(dir)?;
        let copies = copy_into(paths, &dir)?;
        self.show_in_tree(&dir)?;
        if !copies.is_empty() {
            self.record(FileOp::Copied(copies.clone()));
        }
        Ok(copies)
    }

    /// Reverses the newest recorded file operation and returns what it was.
    /// Created entries and copies go to the Recycle Bin / Trash (nothing is
    /// lost); renames and moves go back where they came from.
    pub fn undo_file_op(&mut self) -> Result<FileOp> {
        let op = self.history.pop().ok_or(CoreError::NothingToUndo)?;
        let fail = |err: CoreError| CoreError::Undo {
            what: op.label(),
            reason: err.to_string(),
        };
        match &op {
            FileOp::Created(path) => self.trash(path).map_err(fail)?,
            FileOp::Copied(paths) => {
                for path in paths {
                    self.trash(path).map_err(fail)?;
                }
            }
            FileOp::Renamed { from, to } => self
                .move_back(&[(from.clone(), to.clone())])
                .map_err(fail)?,
            FileOp::Moved(moves) => self.move_back(moves).map_err(fail)?,
        }
        Ok(op)
    }

    /// Whether `undo_file_op` has something to undo.
    pub fn can_undo_file_op(&self) -> bool {
        !self.history.is_empty()
    }

    /// Moves each `to` back to its `from` (newest first); refuses if a `from`
    /// is taken again or a `to` is gone.
    fn move_back(&mut self, moves: &[(PathBuf, PathBuf)]) -> Result<()> {
        for (from, to) in moves.iter().rev() {
            if from.symlink_metadata().is_ok() && !file_ops_same(from, to) {
                return Err(CoreError::AlreadyExists(from.clone()));
            }
            std::fs::rename(to, from).map_err(|err| CoreError::io(to, err))?;
            self.moved(to, from);
            self.refresh_parent(to)?;
            self.refresh_parent(from)?;
        }
        Ok(())
    }

    fn moved(&mut self, from: &Path, to: &Path) {
        self.buffers.moved(from, to);
        self.marks.moved(from, to);
    }

    fn record(&mut self, op: FileOp) {
        self.history.push(op);
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
    }

    /// Git state of the workspace, once a status run has finished.
    pub fn git(&self) -> Option<&GitStatusMap> {
        self.git.as_ref()
    }

    /// Stores a `git_status` result (computed off the UI thread).
    pub fn set_git(&mut self, git: Option<GitStatusMap>) {
        self.git = git;
    }

    /// Moves a workspace entry to the Recycle Bin / Trash. Open buffers stay
    /// open (unsaved work is not lost).
    pub fn trash(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = self.resolve_entry(path)?;
        move_to_trash(&path)?;
        self.refresh_parent(&path)
    }
}

/// Same entry under two spellings (a case-only rename on Windows).
fn file_ops_same(a: &Path, b: &Path) -> bool {
    matches!((dunce::canonicalize(a), dunce::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}

impl Core {
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
