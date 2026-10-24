//! ACP sessions against the fake agent (`src/bin/carbon-fake-acp-agent.rs`):
//! start, prompt with streaming and usage, permission requests answered by
//! Carbon, cancel, a crash, sign-in required, a missing program, modes.

use std::path::PathBuf;
use std::time::Duration;

use carbon_core::acp::{
    AcpEvent, AcpManager, AgentProgram, FailureKind, PermissionMode, SessionId, SessionOptions,
    TranscriptItem,
};

fn fake(scenario: &str) -> AgentProgram {
    AgentProgram {
        name: "Fake".into(),
        command: env!("CARGO_BIN_EXE_carbon-fake-acp-agent").into(),
        args: Vec::new(),
        env: vec![("FAKE_ACP".into(), scenario.into())],
        install_hint: None,
        npm: None,
    }
}

fn options(mode: PermissionMode) -> SessionOptions {
    SessionOptions {
        owner: "acme.flows".into(),
        cwd: std::env::temp_dir(),
        mode,
    }
}

/// The next event of `session`, skipping others.
fn next(events: &async_channel::Receiver<AcpEvent>, session: SessionId) -> AcpEvent {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(!left.is_zero(), "no event in time");
        let event = std::thread::scope(|s| {
            s.spawn(|| events.recv_blocking())
                .join()
                .unwrap()
                .expect("open channel")
        });
        if event.session() == session {
            return event;
        }
    }
}

/// Events until `PromptFinished`/`Failed`/`Closed`, collected.
fn until_end(events: &async_channel::Receiver<AcpEvent>, session: SessionId) -> Vec<AcpEvent> {
    let mut out = Vec::new();
    loop {
        let event = next(events, session);
        let end = matches!(
            event,
            AcpEvent::PromptFinished { .. } | AcpEvent::Failed { .. } | AcpEvent::Closed { .. }
        );
        out.push(event);
        if end {
            return out;
        }
    }
}

fn ready(events: &async_channel::Receiver<AcpEvent>, session: SessionId) {
    loop {
        match next(events, session) {
            AcpEvent::Starting { .. } => continue,
            AcpEvent::Ready { owner, .. } => {
                assert_eq!(owner, "acme.flows");
                return;
            }
            other => panic!("expected Ready, got {other:?}"),
        }
    }
}

#[test]
fn a_prompt_streams_text_and_reports_usage() {
    let (manager, events) = AcpManager::new(None);
    let id = manager.start_session(fake("ok"), options(PermissionMode::Ask));
    ready(&events, id);
    manager.prompt(id, "hi".into()).unwrap();
    let seen = until_end(&events, id);
    let texts: Vec<_> = seen
        .iter()
        .filter_map(|e| match e {
            AcpEvent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["Hello ", "world"]);
    match seen.last().unwrap() {
        AcpEvent::PromptFinished {
            stop_reason,
            text,
            usage: Some(usage),
            truncated: false,
            ..
        } => {
            assert_eq!(stop_reason, "end_turn");
            assert_eq!(text, "Hello world");
            assert_eq!(
                (usage.input_tokens, usage.output_tokens, usage.cached_tokens),
                (100, 20, 12)
            );
            assert_eq!(usage.cost_usd, Some(0.25));
        }
        other => panic!("{other:?}"),
    }
    // Two messages in one answer are kept apart.
    manager.prompt(id, "twice".into()).unwrap();
    match until_end(&events, id).last().unwrap() {
        AcpEvent::PromptFinished { text, .. } => assert_eq!(text, "First.\n\nSecond."),
        other => panic!("{other:?}"),
    }
    let transcript = manager.transcript(id);
    assert!(transcript.contains(&TranscriptItem::Prompt("hi".into())));
    assert!(transcript.contains(&TranscriptItem::Agent("Hello world".into())));
    manager.close(id);
    assert!(matches!(next(&events, id), AcpEvent::Closed { .. }));
    // The transcript stays after the end.
    assert!(!manager.transcript(id).is_empty());
    assert!(!manager.is_live(id));
}

#[test]
fn carbon_answers_permission_requests() {
    let (manager, events) = AcpManager::new(None);
    let id = manager.start_session(fake("permission"), options(PermissionMode::Ask));
    ready(&events, id);
    manager.prompt(id, "test".into()).unwrap();
    let request = loop {
        if let AcpEvent::Permission {
            request,
            title,
            kind,
            options,
            ..
        } = next(&events, id)
        {
            assert_eq!(title, "Run cargo test");
            assert_eq!(kind, "execute");
            assert_eq!(options.len(), 2);
            break request;
        }
    };
    manager.answer_permission(id, request, Some("reject".into()));
    match until_end(&events, id).last().unwrap() {
        AcpEvent::PromptFinished { text, .. } => assert_eq!(text, "permission: reject"),
        other => panic!("{other:?}"),
    }
    assert!(manager.transcript(id).iter().any(|item| matches!(
        item,
        TranscriptItem::Permission { answer, .. } if answer == "reject"
    )));
    // Cancelling while a request waits refuses it.
    manager.prompt(id, "again".into()).unwrap();
    while !matches!(next(&events, id), AcpEvent::Permission { .. }) {}
    manager.cancel(id);
    match until_end(&events, id).last().unwrap() {
        AcpEvent::PromptFinished { text, .. } => assert_eq!(text, "permission: cancelled"),
        other => panic!("{other:?}"),
    }
    manager.close(id);
}

#[test]
fn cancel_ends_a_running_prompt() {
    let (manager, events) = AcpManager::new(None);
    let id = manager.start_session(fake("hang"), options(PermissionMode::Ask));
    ready(&events, id);
    manager.prompt(id, "forever".into()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    manager.cancel(id);
    match until_end(&events, id).last().unwrap() {
        AcpEvent::PromptFinished { stop_reason, .. } => assert_eq!(stop_reason, "cancelled"),
        other => panic!("{other:?}"),
    }
    manager.close_owner("acme.flows");
    assert!(matches!(next(&events, id), AcpEvent::Closed { .. }));
}

#[test]
fn failures_say_why() {
    let (manager, events) = AcpManager::new(None);
    let crash = manager.start_session(fake("crash"), options(PermissionMode::Ask));
    ready(&events, crash);
    manager.prompt(crash, "go".into()).unwrap();
    match until_end(&events, crash).last().unwrap() {
        AcpEvent::Failed { kind, message, .. } => {
            assert_eq!(*kind, FailureKind::Crashed);
            assert!(message.contains("crashing"), "{message}");
        }
        other => panic!("{other:?}"),
    }

    let auth = manager.start_session(fake("auth"), options(PermissionMode::Ask));
    match until_end(&events, auth).last().unwrap() {
        AcpEvent::Failed { kind, .. } => assert_eq!(*kind, FailureKind::AuthRequired),
        other => panic!("{other:?}"),
    }

    let mut missing = fake("ok");
    missing.command = "carbon-no-such-agent".into();
    missing.install_hint = Some("Install it first.".into());
    let gone = manager.start_session(missing, options(PermissionMode::Ask));
    match until_end(&events, gone).last().unwrap() {
        AcpEvent::Failed { kind, message, .. } => {
            assert_eq!(*kind, FailureKind::MissingProgram);
            assert!(message.contains("Install it first."), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn modes_are_set_and_capped() {
    let (manager, events) = AcpManager::new(Some(PathBuf::from("unused")));
    // The user's cap wins over what the flow asks for.
    manager.set_max_mode(PermissionMode::AllowEdits);
    let id = manager.start_session(fake("ok"), options(PermissionMode::AllowAll));
    ready(&events, id);
    let transcript = manager.transcript(id);
    assert!(
        transcript
            .iter()
            .any(|i| matches!(i, TranscriptItem::Note(n) if n.contains("allow-edits"))),
        "{transcript:?}"
    );
    manager.close(id);
}

/// The real Claude Code adapter, installed from npm into a scratch tools
/// folder and run with node: start, one short prompt, close. Needs Node.js,
/// npm, the network and a Claude Code sign-in; costs one small prompt.
#[test]
#[ignore = "installs the Claude Code ACP adapter and sends a real prompt"]
fn claude_code_through_a_managed_npm_install() {
    let tools = tempfile::tempdir().unwrap();
    let (manager, events) = AcpManager::new(Some(tools.path().to_path_buf()));
    let program = AgentProgram {
        name: "Claude Code".into(),
        // Not on PATH: the npm install is used.
        command: "carbon-test-not-on-path".into(),
        args: Vec::new(),
        env: Vec::new(),
        install_hint: None,
        npm: Some(carbon_core::extension::NpmDef {
            package: "@agentclientprotocol/claude-agent-acp".into(),
            version: "0.95.0".into(),
            bin: "claude-agent-acp".into(),
            npx_fallback: false,
        }),
    };
    let work = tempfile::tempdir().unwrap();
    let id = manager.start_session(
        program,
        SessionOptions {
            owner: "test".into(),
            cwd: work.path().to_path_buf(),
            mode: PermissionMode::Ask,
        },
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    loop {
        assert!(std::time::Instant::now() < deadline);
        match events.recv_blocking().unwrap() {
            AcpEvent::Ready { .. } => break,
            AcpEvent::Starting { message, .. } => eprintln!("{message}"),
            AcpEvent::Failed { message, .. } => panic!("{message}"),
            _ => {}
        }
    }
    manager
        .prompt(
            id,
            "Reply with exactly the word pong. Do not use tools.".into(),
        )
        .unwrap();
    loop {
        match events.recv_blocking().unwrap() {
            AcpEvent::PromptFinished { text, usage, .. } => {
                eprintln!("answer: {text:?}, usage: {usage:?}");
                assert!(text.to_lowercase().contains("pong"));
                break;
            }
            AcpEvent::Failed { message, .. } => panic!("{message}"),
            _ => {}
        }
    }
    manager.close(id);
}
