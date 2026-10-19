//! Chats: conversations with an agent backend, as plain data the chat panel
//! shows and saves. Where replies come from is a [`ChatBackend`]; the only one
//! built in is [`DemoBackend`], which streams a canned answer (no model or API
//! is wired up yet, `project/not_built/README.md`).
//!
//! Agent sessions with tool calls and approvals (`agent.rs`) are separate: a
//! chat is the conversation only.

mod backend;
mod demo;
mod store;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub use crate::agent::Role;
pub use backend::{CancelFlag, ChatBackend, Reply, ReplyEvent, ReplyRequest};
pub use demo::{DemoBackend, chunks, demo_reply};
pub use store::{ChatStore, load_chat, save_chat};

/// Version of the saved chat format.
pub const CHAT_VERSION: u32 = 1;
/// Longest derived chat title, in characters.
pub const TITLE_MAX_CHARS: usize = 48;
/// Most selection text one context item carries.
pub const SELECTION_MAX_BYTES: usize = 20_000;

/// A chat's stable id: kept in the layout and used as its file name, so it
/// survives restarts (unlike the per-run ids of terminals and sessions).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChatId(String);

impl ChatId {
    /// A new id: the time in milliseconds and a counter, in hex.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        Self(format!(
            "{millis:x}-{:x}",
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// An id read back from a layout or file name. Only `[0-9a-z-]`, 1 to 64
    /// characters: it becomes a file name, so nothing else gets through.
    pub fn parse(text: &str) -> Option<Self> {
        let ok = (1..=64).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-');
        ok.then(|| Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn file_name(&self) -> String {
        format!("{}.json", self.0)
    }
}

impl std::fmt::Display for ChatId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A message's id, unique within its chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContextKind {
    File,
    Selection,
}

/// Something attached to a user message: a whole file or a selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextItem {
    pub kind: ContextKind,
    /// `None` for an unsaved buffer.
    #[serde(default)]
    pub path: Option<PathBuf>,
    /// Workspace-relative path with `/`, or the buffer's name.
    pub label: String,
    /// First and last line, 1-based (selections).
    #[serde(default)]
    pub lines: Option<(u32, u32)>,
    /// The selected text (selections), at most `SELECTION_MAX_BYTES`.
    #[serde(default)]
    pub text: Option<String>,
}

impl ContextItem {
    pub fn file(path: Option<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            kind: ContextKind::File,
            path,
            label: label.into(),
            lines: None,
            text: None,
        }
    }

    pub fn selection(
        path: Option<PathBuf>,
        label: impl Into<String>,
        lines: (u32, u32),
        text: &str,
    ) -> Self {
        let mut end = text.len().min(SELECTION_MAX_BYTES);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        Self {
            kind: ContextKind::Selection,
            path,
            label: label.into(),
            lines: Some(lines),
            text: Some(text[..end].to_string()),
        }
    }

    /// Short label for a chip: the file name, with the lines of a selection.
    pub fn chip_label(&self) -> String {
        let name = self.label.rsplit('/').next().unwrap_or(&self.label);
        match self.lines {
            Some((first, last)) if first == last => format!("{name}:{first}"),
            Some((first, last)) => format!("{name}:{first}-{last}"),
            None => name.to_string(),
        }
    }
}

/// Adds `item` unless the same item is already there; returns whether it was added.
pub fn add_context(list: &mut Vec<ContextItem>, item: ContextItem) -> bool {
    if list.contains(&item) {
        return false;
    }
    list.push(item);
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageState {
    Done,
    /// A reply still coming in.
    Streaming,
    /// A reply the user stopped; its text is what arrived.
    Stopped,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: MessageId,
    pub role: Role,
    pub text: String,
    #[serde(default)]
    pub context: Vec<ContextItem>,
    pub state: MessageState,
}

/// What a chat is doing, from its messages (never stored).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatStatus {
    Idle,
    Responding,
    Stopped,
    Failed(String),
}

impl ChatStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ChatStatus::Idle => "Idle",
            ChatStatus::Responding => "Responding",
            ChatStatus::Stopped => "Stopped",
            ChatStatus::Failed(_) => "Failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chat {
    pub version: u32,
    pub id: ChatId,
    /// From the first user message; `None` while the chat is empty.
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    messages: Vec<ChatMessage>,
    #[serde(default)]
    next_id: u64,
}

impl Chat {
    pub fn new(id: ChatId) -> Self {
        Self {
            version: CHAT_VERSION,
            id,
            title: None,
            messages: Vec::new(),
            next_id: 0,
        }
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn message(&self, id: MessageId) -> Option<&ChatMessage> {
        self.messages.iter().find(|message| message.id == id)
    }

    pub fn index_of(&self, id: MessageId) -> Option<usize> {
        self.messages.iter().position(|message| message.id == id)
    }

    pub fn is_responding(&self) -> bool {
        self.messages
            .iter()
            .any(|message| message.state == MessageState::Streaming)
    }

    pub fn status(&self) -> ChatStatus {
        if self.is_responding() {
            return ChatStatus::Responding;
        }
        let last_reply = self
            .messages
            .iter()
            .rev()
            .find(|message| message.role == Role::Agent);
        match last_reply.map(|message| &message.state) {
            Some(MessageState::Stopped) => ChatStatus::Stopped,
            Some(MessageState::Failed(reason)) => ChatStatus::Failed(reason.clone()),
            _ => ChatStatus::Idle,
        }
    }

    /// Whether `retry` would do something.
    pub fn can_retry(&self) -> bool {
        !self.is_responding()
            && self
                .messages
                .last()
                .is_some_and(|message| message.role == Role::Agent)
    }

    fn push(
        &mut self,
        role: Role,
        text: String,
        context: Vec<ContextItem>,
        state: MessageState,
    ) -> MessageId {
        let id = MessageId(self.next_id);
        self.next_id += 1;
        self.messages.push(ChatMessage {
            id,
            role,
            text,
            context,
            state,
        });
        id
    }

    /// Adds the user's message and an empty reply to fill; returns the
    /// reply's id. Refused (`None`) for blank text or while a reply runs.
    pub fn send(&mut self, text: &str, context: Vec<ContextItem>) -> Option<MessageId> {
        let text = text.trim();
        if text.is_empty() || self.is_responding() {
            return None;
        }
        if self.title.is_none() {
            self.title = Some(title_from(text));
        }
        self.push(Role::User, text.to_string(), context, MessageState::Done);
        Some(self.push(
            Role::Agent,
            String::new(),
            Vec::new(),
            MessageState::Streaming,
        ))
    }

    fn streaming_mut(&mut self, reply: MessageId) -> Option<&mut ChatMessage> {
        self.messages
            .iter_mut()
            .find(|message| message.id == reply && message.state == MessageState::Streaming)
    }

    /// Appends streamed text; false when `reply` is not streaming any more.
    pub fn push_delta(&mut self, reply: MessageId, delta: &str) -> bool {
        match self.streaming_mut(reply) {
            Some(message) => {
                message.text.push_str(delta);
                true
            }
            None => false,
        }
    }

    pub fn finish(&mut self, reply: MessageId) {
        if let Some(message) = self.streaming_mut(reply) {
            message.state = MessageState::Done;
        }
    }

    /// Ends a reply early; the text that arrived stays.
    pub fn stop(&mut self, reply: MessageId) {
        if let Some(message) = self.streaming_mut(reply) {
            message.state = MessageState::Stopped;
        }
    }

    pub fn fail(&mut self, reply: MessageId, reason: impl Into<String>) {
        if let Some(message) = self.streaming_mut(reply) {
            message.state = MessageState::Failed(reason.into());
        }
    }

    /// Replaces the last reply with a new empty one; returns its id.
    pub fn retry(&mut self) -> Option<MessageId> {
        if !self.can_retry() {
            return None;
        }
        self.messages.pop();
        Some(self.push(
            Role::Agent,
            String::new(),
            Vec::new(),
            MessageState::Streaming,
        ))
    }

    /// Replaces user message `user` (and everything after it) with new text
    /// and context, and asks again; returns the new reply's id.
    pub fn edit_and_resend(
        &mut self,
        user: MessageId,
        text: &str,
        context: Vec<ContextItem>,
    ) -> Option<MessageId> {
        let ix = self.index_of(user)?;
        if self.messages[ix].role != Role::User || self.is_responding() || text.trim().is_empty() {
            return None;
        }
        self.messages.truncate(ix);
        if ix == 0 {
            self.title = None;
        }
        self.send(text, context)
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.title = None;
    }

    /// The conversation a backend answers for `reply`: every message before it.
    pub fn request_for(&self, reply: MessageId) -> ReplyRequest {
        let end = self.index_of(reply).unwrap_or(self.messages.len());
        ReplyRequest {
            chat: self.id.clone(),
            messages: self.messages[..end].to_vec(),
        }
    }

    /// A reply cut off by quitting comes back stopped, not streaming forever.
    pub fn normalize_after_load(&mut self) {
        for message in &mut self.messages {
            if message.state == MessageState::Streaming {
                message.state = MessageState::Stopped;
            }
        }
        if let Some(max) = self.messages.iter().map(|message| message.id.0).max() {
            self.next_id = self.next_id.max(max + 1);
        }
    }
}

/// A chat title from its first message: first non-empty line, spaces
/// collapsed, at most `TITLE_MAX_CHARS` characters.
pub fn title_from(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let words = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.chars().count() <= TITLE_MAX_CHARS {
        return words;
    }
    let cut: String = words.chars().take(TITLE_MAX_CHARS).collect();
    format!("{}…", cut.trim_end())
}
