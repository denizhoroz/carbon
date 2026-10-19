//! Chats in memory and on disk (one JSON file per chat).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Chat, ChatBackend, ChatId, DemoBackend, MessageId, Reply};
use crate::{CoreError, Result};

/// Every chat opened this run, the backend that answers them, and where
/// they are saved. Chats stay in memory for the whole run, so a panel
/// rebuilt from a layout finds the live conversation, not a stale file.
pub struct ChatStore {
    chats: HashMap<ChatId, Chat>,
    backend: Arc<dyn ChatBackend>,
    dir: Option<PathBuf>,
}

impl Default for ChatStore {
    fn default() -> Self {
        Self {
            chats: HashMap::new(),
            backend: Arc::new(DemoBackend::default()),
            dir: None,
        }
    }
}

impl ChatStore {
    /// Where chats are saved; without one they live in memory only.
    pub fn set_dir(&mut self, dir: impl Into<PathBuf>) {
        self.dir = Some(dir.into());
    }

    pub fn set_backend(&mut self, backend: Arc<dyn ChatBackend>) {
        self.backend = backend;
    }

    pub fn backend(&self) -> &Arc<dyn ChatBackend> {
        &self.backend
    }

    /// A new empty chat.
    pub fn create(&mut self) -> ChatId {
        let id = ChatId::new();
        self.chats.insert(id.clone(), Chat::new(id.clone()));
        id
    }

    /// The chat `id`: from memory, else from its file, else a new empty one
    /// with that id. A broken file is reported and the chat starts empty.
    pub fn open(&mut self, id: &ChatId) -> &mut Chat {
        if !self.chats.contains_key(id) {
            let loaded = match &self.dir {
                Some(dir) => load_chat(dir, id).unwrap_or_else(|err| {
                    eprintln!("cannot load chat {id}: {err}; starting empty");
                    None
                }),
                None => None,
            };
            self.chats
                .insert(id.clone(), loaded.unwrap_or_else(|| Chat::new(id.clone())));
        }
        self.chats.get_mut(id).expect("inserted above")
    }

    pub fn get(&self, id: &ChatId) -> Option<&Chat> {
        self.chats.get(id)
    }

    pub fn get_mut(&mut self, id: &ChatId) -> Option<&mut Chat> {
        self.chats.get_mut(id)
    }

    /// Writes chat `id` to its file (nothing without a directory).
    pub fn save(&self, id: &ChatId) -> Result<()> {
        match (&self.dir, self.chats.get(id)) {
            (Some(dir), Some(chat)) => save_chat(dir, chat),
            _ => Ok(()),
        }
    }

    /// Asks the backend for `reply` in chat `id`.
    pub fn start_reply(&self, id: &ChatId, reply: MessageId) -> Option<Reply> {
        let request = self.chats.get(id)?.request_for(reply);
        Some(self.backend.reply(request))
    }
}

/// Writes `chat` to `<dir>/<id>.json` (via a temporary file, so a crash
/// mid-write leaves the old file).
pub fn save_chat(dir: &Path, chat: &Chat) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|err| CoreError::io(dir, err))?;
    let json =
        serde_json::to_string_pretty(chat).map_err(|err| CoreError::Chat(err.to_string()))?;
    let path = dir.join(chat.id.file_name());
    let tmp = dir.join(format!("{}.tmp", chat.id.file_name()));
    std::fs::write(&tmp, json).map_err(|err| CoreError::io(&tmp, err))?;
    std::fs::rename(&tmp, &path).map_err(|err| CoreError::io(&path, err))
}

/// Reads `<dir>/<id>.json`; `None` when there is no such file.
pub fn load_chat(dir: &Path, id: &ChatId) -> Result<Option<Chat>> {
    let path = dir.join(id.file_name());
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(CoreError::io(&path, err)),
    };
    let mut chat: Chat = serde_json::from_str(&text)
        .map_err(|err| CoreError::Chat(format!("{}: {err}", path.display())))?;
    if chat.id != *id {
        return Err(CoreError::Chat(format!(
            "{}: holds chat {}",
            path.display(),
            chat.id
        )));
    }
    chat.normalize_after_load();
    Ok(Some(chat))
}
