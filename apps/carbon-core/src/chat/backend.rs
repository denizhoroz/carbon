//! Where replies come from. A backend gets the conversation and streams the
//! answer back through a channel; the chat panel reads it on the UI side.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{ChatId, ChatMessage};

/// The conversation to answer: every message up to and including the last
/// user message (with its context items).
#[derive(Clone, Debug)]
pub struct ReplyRequest {
    pub chat: ChatId,
    pub messages: Vec<ChatMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplyEvent {
    /// More reply text (Markdown).
    Delta(String),
    Done,
    Failed(String),
}

/// Set when the reader no longer wants the reply (Stop, tab closed).
#[derive(Clone, Debug, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A reply being streamed. Dropping it cancels the backend's work.
pub struct Reply {
    events: async_channel::Receiver<ReplyEvent>,
    cancel: CancelFlag,
}

impl Reply {
    pub fn new(events: async_channel::Receiver<ReplyEvent>, cancel: CancelFlag) -> Self {
        Self { events, cancel }
    }

    pub fn events(&self) -> async_channel::Receiver<ReplyEvent> {
        self.events.clone()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Answers chats. A real backend (a local model, an API, an ACP agent)
/// implements `reply`: start the work off the caller's thread, send `Delta`s,
/// then `Done` or `Failed`, and stop early when the cancel flag is set or the
/// channel is closed.
pub trait ChatBackend: Send + Sync {
    /// Shown in the chat header.
    fn name(&self) -> &str;

    /// A stand-in that gives canned answers (the UI says so).
    fn is_demo(&self) -> bool {
        false
    }

    fn reply(&self, request: ReplyRequest) -> Reply;
}
