//! Language server client against a real rust-analyzer. Ignored by default:
//! it needs rust-analyzer installed and takes a while (cargo metadata).
//! Run with `cargo test -p carbon-core --test lsp -- --ignored`.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use carbon_core::Core;
use carbon_core::lsp::{self, LanguageRule, LspStatus, ServerConfig, ServerTable, lsp_types};

/// The table the Rust extension would give: `.rs` files, rust-analyzer.
fn rust_table() -> ServerTable {
    ServerTable {
        languages: vec![LanguageRule {
            language: "rust".into(),
            suffixes: vec!["rs".into()],
            file_names: Vec::new(),
        }],
        servers: vec![Arc::new(ServerConfig {
            language: "rust".into(),
            name: "rust-analyzer".into(),
            command: "rust-analyzer".into(),
            args: Vec::new(),
            env: Default::default(),
            install_hint: "rustup component add rust-analyzer".into(),
            extension: "carbon.rust".into(),
            server_id: "rust-analyzer".into(),
            program: None,
            downloaded: None,
            download: None,
        })],
    }
}

/// Runs a future to completion on this thread.
fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park_timeout(Duration::from_millis(50)),
        }
    }
}

#[test]
#[ignore = "needs rust-analyzer; slow"]
fn rust_analyzer_publishes_a_type_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/main.rs"),
        "fn main() {\n    let x: u32 = \"text\";\n}\n",
    )
    .unwrap();

    let mut core = Core::new();
    core.lsp.set_table(rust_table());
    core.open_workspace(dir.path()).unwrap();
    let root = core.workspace().unwrap().root().to_path_buf();
    let main = root.join("src").join("main.rs");
    let config = core.lsp.server_for_path(&main).expect("rust has a server");
    let server = core.lsp.start(config, &root).expect("rust-analyzer starts");
    let result = block_on(lsp::initialize(server.clone(), &root)).unwrap();
    assert!(result.capabilities.hover_provider.is_some());
    core.lsp.set_status("rust", LspStatus::Ready);

    let id = core.open_file(&main).unwrap();
    assert!(core.lsp.document(id).is_some(), "opened in the server");

    // Wait for a publish for main.rs that has an error.
    let notifications = server.notifications();
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut errors = 0;
    while Instant::now() < deadline && errors == 0 {
        let Ok(message) = notifications.recv_blocking() else {
            panic!("server stopped: {}", server.stderr_tail());
        };
        if message.method != "textDocument/publishDiagnostics" {
            continue;
        }
        let params: lsp_types::PublishDiagnosticsParams =
            serde_json::from_value(message.params).unwrap();
        if lsp::uri_to_path(&params.uri).as_deref() == Some(main.as_path()) {
            core.set_diagnostics(main.clone(), params.diagnostics);
            errors = core.marks.file(&main).errors;
        }
    }
    assert!(errors > 0, "rust-analyzer reported the type error");
    assert!(core.lsp.diagnostics(&main).is_some());
}

#[test]
fn positions_count_utf16_units() {
    use carbon_core::Rope;
    use lsp_types::Position;
    // "é" is 2 bytes and 1 UTF-16 unit; "😀" is 4 bytes and 2 units.
    let rope = Rope::from_str("aé😀b\r\nx");
    let b = "aé😀".len();
    assert_eq!(lsp::offset_to_position(&rope, b), Position::new(0, 4));
    assert_eq!(lsp::position_to_offset(&rope, Position::new(0, 4)), b);
    let x = rope.len() - 1;
    assert_eq!(lsp::offset_to_position(&rope, x), Position::new(1, 0));
    assert_eq!(lsp::position_to_offset(&rope, Position::new(1, 0)), x);
    assert_eq!(
        lsp::position_to_offset(&rope, Position::new(9, 9)),
        rope.len()
    );
}

#[test]
fn files_get_the_server_of_their_language() {
    use std::path::Path;
    let table = rust_table();
    assert_eq!(
        table.language_for_path(Path::new("src/MAIN.RS")),
        Some("rust")
    );
    assert_eq!(
        table
            .server_for_path(Path::new("src/main.rs"))
            .unwrap()
            .name,
        "rust-analyzer"
    );
    assert!(table.server_for_path(Path::new("notes.txt")).is_none());
    assert!(
        ServerTable::default()
            .server_for_path(Path::new("a.rs"))
            .is_none()
    );

    // The table survives opening a folder; a changed table drops servers
    // whose config changed.
    let mut core = Core::new();
    core.lsp.set_table(table.clone());
    let dir = tempfile::tempdir().unwrap();
    core.open_workspace(dir.path()).unwrap();
    assert_eq!(core.lsp.table(), &table);
    core.lsp.set_table(ServerTable::default());
    assert!(core.lsp.server_for_path(Path::new("a.rs")).is_none());
}
