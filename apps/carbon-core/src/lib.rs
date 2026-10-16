//! Carbon core: the state the UI panels show, as plain Rust with no UI dependency.
//!
//! In-process for now (PROJECT.md, option a); the API is shaped so it can move
//! behind a daemon later. Long-running work (terminal output) is reported through
//! `async_channel` receivers, which work from both threads and async executors.

mod agent;
mod buffer;
mod error;
mod file_tree;
mod id;
mod jobs;
mod terminal;
mod vcs;
mod workspace;

use std::path::Path;

pub use agent::{AgentSession, AgentSessionId, AgentStatus, AgentStore, Message, Role};
pub use buffer::{Buffer, BufferId, BufferStore, LineEnding};
pub use error::{CoreError, Result};
pub use file_tree::{EntryKind, FileEntry, list_dir};
pub use jobs::{Job, JobId, JobStatus, JobStore};
pub use ropey::Rope;
pub use terminal::{Terminal, TerminalEvent, TerminalId, TerminalSpec, TerminalStore};
pub use vcs::{Branch, current_branch};
pub use workspace::Workspace;

/// Everything one window works on: the open folder and its sessions.
#[derive(Default)]
pub struct Core {
    workspace: Option<Workspace>,
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
        Ok(self.workspace.insert(Workspace::open(path)?))
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

    /// Opens a workspace file into a buffer (or returns the already open one).
    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<BufferId> {
        let workspace = self.workspace.as_ref().ok_or(CoreError::NoWorkspace)?;
        let path = workspace.resolve(path)?;
        self.buffers.open(path)
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
