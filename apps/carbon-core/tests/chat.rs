use std::collections::HashSet;
use std::time::Duration;

use carbon_core::chat::{
    Chat, ChatBackend, ChatId, ChatStatus, ChatStore, ContextItem, ContextKind, DemoBackend,
    MessageState, ReplyEvent, Role, SELECTION_MAX_BYTES, TITLE_MAX_CHARS, add_context, chunks,
    demo_reply, load_chat, save_chat, title_from,
};

fn chat() -> Chat {
    Chat::new(ChatId::new())
}

#[test]
fn send_adds_user_message_and_streaming_reply() {
    let mut chat = chat();
    assert_eq!(chat.status(), ChatStatus::Idle);
    assert_eq!(chat.send("   ", Vec::new()), None, "blank text is refused");

    let reply = chat.send("Hello there", Vec::new()).unwrap();
    let messages = chat.messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].text, "Hello there");
    assert_eq!(messages[1].id, reply);
    assert_eq!(messages[1].state, MessageState::Streaming);
    assert_eq!(chat.status(), ChatStatus::Responding);
    assert_eq!(chat.send("again", Vec::new()), None, "one reply at a time");
}

#[test]
fn deltas_then_finish() {
    let mut chat = chat();
    let reply = chat.send("Explain\nmore lines", Vec::new()).unwrap();
    assert!(chat.push_delta(reply, "Hi "));
    assert!(chat.push_delta(reply, "there"));
    chat.finish(reply);
    assert!(
        !chat.push_delta(reply, "late"),
        "a finished reply takes no more text"
    );
    let last = chat.messages().last().unwrap();
    assert_eq!(last.text, "Hi there");
    assert_eq!(last.state, MessageState::Done);
    assert_eq!(chat.status(), ChatStatus::Idle);
    assert_eq!(chat.title.as_deref(), Some("Explain"));
}

#[test]
fn titles_are_first_line_and_capped() {
    assert_eq!(title_from("\n  hello   world \nnext"), "hello world");
    let long = "ä".repeat(TITLE_MAX_CHARS + 10);
    let title = title_from(&long);
    assert_eq!(title.chars().count(), TITLE_MAX_CHARS + 1);
    assert!(title.ends_with('…'));
}

#[test]
fn stop_keeps_partial_and_fail_keeps_reason() {
    let mut chat = chat();
    let reply = chat.send("one", Vec::new()).unwrap();
    chat.push_delta(reply, "partial");
    chat.stop(reply);
    assert_eq!(chat.messages()[1].text, "partial");
    assert_eq!(chat.messages()[1].state, MessageState::Stopped);
    assert_eq!(chat.status(), ChatStatus::Stopped);

    let reply = chat.send("two", Vec::new()).unwrap();
    chat.fail(reply, "boom");
    assert_eq!(chat.status(), ChatStatus::Failed("boom".into()));
    assert_eq!(chat.status().label(), "Failed");
}

#[test]
fn retry_replaces_the_last_reply() {
    let mut chat = chat();
    let first = chat.send("q", Vec::new()).unwrap();
    assert_eq!(chat.retry(), None, "not while responding");
    chat.push_delta(first, "old");
    chat.finish(first);

    let second = chat.retry().unwrap();
    assert_ne!(second, first);
    assert_eq!(chat.messages().len(), 2);
    assert_eq!(chat.messages()[1].text, "");
    assert_eq!(chat.messages()[1].state, MessageState::Streaming);
}

#[test]
fn edit_and_resend_drops_later_turns() {
    let mut chat = chat();
    let r1 = chat.send("first", Vec::new()).unwrap();
    chat.finish(r1);
    let r2 = chat.send("second", Vec::new()).unwrap();
    chat.finish(r2);
    let first_user = chat.messages()[0].id;

    let file = ContextItem::file(None, "src/main.rs");
    let reply = chat
        .edit_and_resend(first_user, "first, edited", vec![file.clone()])
        .unwrap();
    let messages = chat.messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text, "first, edited");
    assert_eq!(messages[0].context, vec![file]);
    assert_eq!(chat.title.as_deref(), Some("first, edited"));

    let request = chat.request_for(reply);
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].text, "first, edited");
    assert_eq!(
        chat.edit_and_resend(reply, "x", Vec::new()),
        None,
        "only user messages"
    );
}

#[test]
fn clear_empties_messages_and_title() {
    let mut chat = chat();
    let reply = chat.send("hi", Vec::new()).unwrap();
    chat.finish(reply);
    chat.clear();
    assert!(chat.messages().is_empty());
    assert_eq!(chat.title, None);
}

#[test]
fn serde_round_trip_and_stopped_after_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut chat = chat();
    let item = ContextItem::selection(None, "a/b.rs", (3, 5), "let x = 1;");
    let reply = chat.send("hi", vec![item]).unwrap();
    chat.push_delta(reply, "half");
    save_chat(dir.path(), &chat).unwrap();

    let loaded = load_chat(dir.path(), &chat.id).unwrap().unwrap();
    assert_eq!(loaded.messages()[0], chat.messages()[0]);
    assert_eq!(loaded.messages()[1].text, "half");
    assert_eq!(loaded.messages()[1].state, MessageState::Stopped);

    // New ids after load do not collide with saved ones.
    let mut loaded = loaded;
    let next = loaded.send("again", Vec::new()).unwrap();
    assert!(next.0 > reply.0);

    assert_eq!(load_chat(dir.path(), &ChatId::new()).unwrap(), None);
    let broken = ChatId::new();
    std::fs::write(dir.path().join(broken.file_name()), "{nope").unwrap();
    assert!(load_chat(dir.path(), &broken).is_err());
}

#[test]
fn store_opens_saves_and_reloads() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ChatStore::default();
    store.set_dir(dir.path());
    let id = store.create();
    let reply = store.open(&id).send("persist me", Vec::new()).unwrap();
    store.get_mut(&id).unwrap().finish(reply);
    store.save(&id).unwrap();

    let mut fresh = ChatStore::default();
    fresh.set_dir(dir.path());
    assert_eq!(fresh.open(&id).messages()[0].text, "persist me");
    let unknown = ChatId::new();
    assert!(fresh.open(&unknown).messages().is_empty());
}

#[test]
fn chat_ids_are_unique_and_safe() {
    let ids: HashSet<String> = (0..1000).map(|_| ChatId::new().to_string()).collect();
    assert_eq!(ids.len(), 1000);
    for id in &ids {
        assert!(ChatId::parse(id).is_some());
    }
    for bad in ["", "../x", "ABC", "a b", &"a".repeat(65), "a/b", "a.json"] {
        assert_eq!(ChatId::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn context_items() {
    let big = "é".repeat(SELECTION_MAX_BYTES);
    let item = ContextItem::selection(None, "src/lib.rs", (10, 24), &big);
    let text = item.text.as_ref().unwrap();
    assert!(text.len() <= SELECTION_MAX_BYTES);
    assert_eq!(item.kind, ContextKind::Selection);
    assert_eq!(item.chip_label(), "lib.rs:10-24");
    assert_eq!(
        ContextItem::selection(None, "a.rs", (4, 4), "x").chip_label(),
        "a.rs:4"
    );
    assert_eq!(
        ContextItem::file(None, "src/main.rs").chip_label(),
        "main.rs"
    );

    let mut list = Vec::new();
    assert!(add_context(&mut list, ContextItem::file(None, "a.rs")));
    assert!(!add_context(&mut list, ContextItem::file(None, "a.rs")));
    assert_eq!(list.len(), 1);
}

#[test]
fn chunks_join_back() {
    for text in [
        "",
        "one",
        "one two three four five",
        "  lead\n\ntrail  ",
        "ünï cødé ✓ words here",
    ] {
        let parts = chunks(text, 2);
        assert_eq!(parts.concat(), text);
    }
    assert_eq!(chunks("a b c d", 2), vec!["a b ", "c d"]);
}

fn drain(reply: &carbon_core::chat::Reply) -> Vec<ReplyEvent> {
    let events = reply.events();
    let mut out = Vec::new();
    while let Ok(event) = events.recv_blocking() {
        let end = !matches!(event, ReplyEvent::Delta(_));
        out.push(event);
        if end {
            break;
        }
    }
    out
}

#[test]
fn demo_streams_the_canned_reply() {
    let backend = DemoBackend::with_timing(Duration::ZERO, Duration::ZERO);
    let mut chat = chat();
    let items = vec![
        ContextItem::file(None, "src/main.rs"),
        ContextItem::selection(None, "src/lib.rs", (1, 2), "x"),
    ];
    let reply = chat.send("Explain this", items).unwrap();
    let request = chat.request_for(reply);
    let expected = demo_reply(&request);
    assert!(expected.contains("2 context items"));
    assert!(expected.contains("```rust"));

    let events = drain(&backend.reply(request));
    assert_eq!(events.last(), Some(&ReplyEvent::Done));
    let text: String = events
        .iter()
        .filter_map(|event| match event {
            ReplyEvent::Delta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, expected);
    assert!(backend.is_demo());
}

#[test]
fn demo_stops_when_cancelled() {
    let backend = DemoBackend::with_timing(Duration::ZERO, Duration::from_millis(20));
    let mut chat = chat();
    let reply = chat.send("hi", Vec::new()).unwrap();
    let running = backend.reply(chat.request_for(reply));
    let events = running.events();
    assert!(matches!(events.recv_blocking(), Ok(ReplyEvent::Delta(_))));
    running.cancel();
    let mut rest = Vec::new();
    while let Ok(event) = events.recv_blocking() {
        rest.push(event);
    }
    assert!(
        !rest.contains(&ReplyEvent::Done),
        "a cancelled reply never finishes"
    );
}

#[test]
fn demo_fails_on_request() {
    let backend = DemoBackend::with_timing(Duration::ZERO, Duration::ZERO);
    let mut chat = chat();
    let reply = chat.send("please /fail", Vec::new()).unwrap();
    let events = drain(&backend.reply(chat.request_for(reply)));
    assert!(matches!(events.last(), Some(ReplyEvent::Failed(_))));
}
