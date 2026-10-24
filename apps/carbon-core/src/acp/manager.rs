//! The sessions Carbon runs: start, prompt, cancel, close, answer permission
//! requests, read transcripts. Every call returns at once; the work happens
//! on each session's thread (`session.rs`) and comes back as events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use super::session::{self, Msg, Setup};
use super::transcript::Transcript;
use super::{AcpEvent, AgentProgram, PermissionMode, TranscriptItem};

pub type SessionId = u64;

/// How to start a session.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    /// Who started it (an extension id); its events carry it.
    pub owner: String,
    /// The folder the agent works in.
    pub cwd: PathBuf,
    pub mode: PermissionMode,
}

struct Handle {
    owner: String,
    commands: Sender<Msg>,
    transcript: Arc<Mutex<Transcript>>,
    /// Ended sessions stay (their transcripts are read after) until
    /// `forget`, or until too many have ended.
    ended: bool,
}

/// Ended sessions kept for their transcripts.
const KEEP_ENDED: usize = 200;

struct Inner {
    next: AtomicU64,
    sessions: Mutex<HashMap<SessionId, Handle>>,
    events: async_channel::Sender<AcpEvent>,
    /// Where npm-installed agent programs go.
    tools: Option<PathBuf>,
    /// The most any session may be allowed (a user setting).
    max_mode: Mutex<PermissionMode>,
}

#[derive(Clone)]
pub struct AcpManager {
    inner: Arc<Inner>,
}

impl AcpManager {
    /// A manager and the receiver of every session's events.
    pub fn new(tools: Option<PathBuf>) -> (Self, async_channel::Receiver<AcpEvent>) {
        let (events, receiver) = async_channel::unbounded();
        let manager = Self {
            inner: Arc::new(Inner {
                next: AtomicU64::new(1),
                sessions: Mutex::new(HashMap::new()),
                events,
                tools,
                max_mode: Mutex::new(PermissionMode::AllowEdits),
            }),
        };
        (manager, receiver)
    }

    /// Caps the mode of sessions started from now on.
    pub fn set_max_mode(&self, mode: PermissionMode) {
        if let Ok(mut max) = self.inner.max_mode.lock() {
            *max = mode;
        }
    }

    pub fn max_mode(&self) -> PermissionMode {
        self.inner.max_mode.lock().map(|m| *m).unwrap_or_default()
    }

    /// Starts a session; `Ready` (or `Failed`) follows as an event.
    pub fn start_session(&self, program: AgentProgram, options: SessionOptions) -> SessionId {
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let mode = options.mode.min(self.max_mode());
        let (tx, rx) = mpsc::channel();
        let transcript = Arc::new(Mutex::new(Transcript::default()));
        if let Ok(mut sessions) = self.inner.sessions.lock() {
            sessions.insert(
                id,
                Handle {
                    owner: options.owner.clone(),
                    commands: tx.clone(),
                    transcript: transcript.clone(),
                    ended: false,
                },
            );
        }
        let setup = Setup {
            id,
            owner: options.owner,
            program,
            cwd: options.cwd,
            mode,
            tools: self.inner.tools.clone(),
        };
        let events = self.inner.events.clone();
        let inner = Arc::downgrade(&self.inner);
        let spawned = std::thread::Builder::new()
            .name(format!("acp-session-{id}"))
            .spawn(move || {
                let last = session::run(setup, rx, tx, events.clone(), transcript);
                if let Some(inner) = inner.upgrade()
                    && let Ok(mut sessions) = inner.sessions.lock()
                {
                    if let Some(handle) = sessions.get_mut(&id) {
                        handle.ended = true;
                    }
                    let mut ended: Vec<SessionId> = sessions
                        .iter()
                        .filter(|(_, h)| h.ended)
                        .map(|(id, _)| *id)
                        .collect();
                    if ended.len() > KEEP_ENDED {
                        ended.sort_unstable();
                        for old in &ended[..ended.len() - KEEP_ENDED] {
                            sessions.remove(old);
                        }
                    }
                }
                _ = events.send_blocking(last);
            });
        if spawned.is_err()
            && let Ok(mut sessions) = self.inner.sessions.lock()
        {
            sessions.remove(&id);
        }
        id
    }

    fn send(&self, id: SessionId, msg: Msg) -> Result<(), String> {
        let sessions = self.inner.sessions.lock().map_err(|_| "poisoned")?;
        let handle = sessions
            .get(&id)
            .ok_or_else(|| format!("no session {id}"))?;
        handle
            .commands
            .send(msg)
            .map_err(|_| format!("session {id} has ended"))
    }

    /// Sends a prompt (queued after the one running); `PromptFinished`
    /// follows.
    pub fn prompt(&self, id: SessionId, text: String) -> Result<(), String> {
        self.send(id, Msg::Prompt(text))
    }

    /// Stops the running prompt (it finishes with `cancelled`) and drops
    /// queued ones; pending permission requests are cancelled.
    pub fn cancel(&self, id: SessionId) {
        _ = self.send(id, Msg::Cancel);
    }

    /// Ends the session and its program.
    pub fn close(&self, id: SessionId) {
        _ = self.send(id, Msg::Close);
    }

    /// Ends every session `owner` started (it was turned off).
    pub fn close_owner(&self, owner: &str) {
        for id in self.sessions_of(owner) {
            self.close(id);
        }
    }

    /// Answers a permission request: an option id, or `None` to refuse.
    pub fn answer_permission(&self, id: SessionId, request: u64, option: Option<String>) {
        _ = self.send(id, Msg::Answer { request, option });
    }

    /// Drops an ended session's transcript.
    pub fn forget(&self, id: SessionId) {
        if let Ok(mut sessions) = self.inner.sessions.lock()
            && sessions.get(&id).is_some_and(|h| h.ended)
        {
            sessions.remove(&id);
        }
    }

    /// Whether the session is still running.
    pub fn is_live(&self, id: SessionId) -> bool {
        self.inner
            .sessions
            .lock()
            .is_ok_and(|s| s.get(&id).is_some_and(|h| !h.ended))
    }

    pub fn owner_of(&self, id: SessionId) -> Option<String> {
        let sessions = self.inner.sessions.lock().ok()?;
        sessions.get(&id).map(|h| h.owner.clone())
    }

    pub fn sessions_of(&self, owner: &str) -> Vec<SessionId> {
        self.inner
            .sessions
            .lock()
            .map(|s| {
                s.iter()
                    .filter(|(_, h)| h.owner == owner && !h.ended)
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What the session showed so far (empty once it has ended and gone).
    pub fn transcript(&self, id: SessionId) -> Vec<TranscriptItem> {
        let Ok(sessions) = self.inner.sessions.lock() else {
            return Vec::new();
        };
        sessions
            .get(&id)
            .and_then(|h| h.transcript.lock().ok().map(|t| t.items.clone()))
            .unwrap_or_default()
    }
}
