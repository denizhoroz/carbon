//! Pure helpers: what a saved panel holds, which placeholder titles become
//! chats, how context chips are labelled, when Enter sends.

use std::path::Path;

use carbon_core::chat::ChatId;
use gpui_kit::SharedString;

/// Label and chat id from a saved panel (`{"label", "chat"}`). A missing or
/// unusable id gives `None` (the panel starts a new chat).
pub fn parse_info(value: &serde_json::Value) -> (SharedString, Option<ChatId>) {
    let label = value["label"]
        .as_str()
        .or_else(|| value["title"].as_str())
        .unwrap_or("Chat");
    let chat = value["chat"].as_str().and_then(ChatId::parse);
    (label.to_string().into(), chat)
}

/// Whether a placeholder or preset title stands for a chat: "Chat",
/// "Chat 2", "Agent Chat", "Agent Chat 1".
pub fn is_chat_title(title: &str) -> bool {
    let base = match title.rsplit_once(' ') {
        Some((base, number)) if number.parse::<u32>().is_ok() => base,
        _ => title,
    };
    base == "Chat" || base == "Agent Chat"
}

/// A context chip's label: the path relative to the workspace with `/`, or
/// the full path outside it.
pub fn relative_label(path: &Path, root: Option<&Path>) -> String {
    let shown = root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    shown
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Enter sends unless Shift is held (a new line), the text is blank, or a
/// reply is still coming in.
pub fn should_send(shift: bool, text: &str, responding: bool) -> bool {
    !shift && !responding && !text.trim().is_empty()
}

#[cfg(test)]
mod tests {
    // Not `super::*` with a gpui glob in scope: GPUI's `test` macro would shadow `#[test]`.
    use std::path::Path;

    use super::{is_chat_title, parse_info, relative_label, should_send};

    #[test]
    fn saved_info() {
        let (label, chat) = parse_info(&serde_json::json!({"label": "Chat 2", "chat": "abc-1"}));
        assert_eq!(label.as_ref(), "Chat 2");
        assert_eq!(chat.unwrap().as_str(), "abc-1");
        let (label, chat) = parse_info(&serde_json::json!({"title": "Chat", "chat": "../x"}));
        assert_eq!(label.as_ref(), "Chat");
        assert!(chat.is_none());
        assert_eq!(parse_info(&serde_json::Value::Null).0.as_ref(), "Chat");
    }

    #[test]
    fn chat_titles() {
        for title in ["Chat", "Chat 2", "Agent Chat", "Agent Chat 1"] {
            assert!(is_chat_title(title), "{title}");
        }
        for title in ["Chatter", "Chat x", "Terminal", "Agent Logs", "My Chat"] {
            assert!(!is_chat_title(title), "{title}");
        }
    }

    #[test]
    fn labels() {
        let root = Path::new("/work/proj");
        assert_eq!(
            relative_label(Path::new("/work/proj/src/main.rs"), Some(root)),
            "src/main.rs"
        );
        // Outside the workspace the full path stays (its root prints per platform).
        let outside = relative_label(Path::new("/elsewhere/a.rs"), Some(root));
        assert!(outside.ends_with("elsewhere/a.rs"), "{outside}");
    }

    #[test]
    fn enter_sends() {
        assert!(should_send(false, "hi", false));
        assert!(!should_send(true, "hi", false), "Shift+Enter is a new line");
        assert!(!should_send(false, "  \n", false), "blank");
        assert!(!should_send(false, "hi", true), "a reply is running");
    }
}
