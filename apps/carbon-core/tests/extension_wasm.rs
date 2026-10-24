//! Extension code: the example guest (`apps/carbon-extension-api/examples/hello`)
//! built for `wasm32-wasip2` and run by the host, with and without grants.
//! Ignored by default: it builds the guest (needs `rustup target add
//! wasm32-wasip2`) and waits out the 5 s time limit once. Run with
//! `cargo test -p carbon-core --test extension_wasm -- --ignored`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use carbon_core::extension::{
    CallError, ExtensionEvent, ExtensionId, HostContext, HostEvent, Registry, StateFile,
    WasmRuntime, Widget,
};

/// Builds the example guest; returns its `.wasm`.
fn build_guest() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("apps/carbon-extension-api/examples/hello");
    let target = root.join("target/extension-examples");
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip2",
            "--target-dir",
        ])
        .arg(&target)
        .current_dir(&example)
        .status()
        .expect("cargo runs");
    assert!(status.success(), "the example guest builds");
    target.join("wasm32-wasip2/release/carbon_hello.wasm")
}

const MANIFEST: &str = r#"
id = "acme.hello"
name = "Hello"
version = "1.0.0"
schema_version = 1
carbon_api = ">=0.2, <0.3"
wasm = "hello.wasm"
capabilities = ["fs.read:package", "fs.write:workspace", "process.exec:git", "net:example.com"]

[[commands]]
id = "count"
title = "Count"

[[panels]]
id = "runs"
title = "Runs"
kind = "view"
view = "runs"
dynamic = true

[[settings]]
key = "greeting"
label = "Greeting"
kind = "text"
default = "hi"
"#;

fn events(context: &HostContext) -> Vec<HostEvent> {
    context.take_events()
}

#[test]
#[ignore = "builds the example guest for wasm32-wasip2; waits 5 s"]
fn the_example_runs_inside_its_grants() {
    let wasm = build_guest();
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("hello");
    fs::create_dir_all(&package).unwrap();
    fs::copy(&wasm, package.join("hello.wasm")).unwrap();
    fs::write(package.join("extension.toml"), MANIFEST).unwrap();
    fs::write(temp.path().join("secret.txt"), "secret").unwrap();
    let mut state = StateFile::default();
    state.dev.insert(package.clone());
    let registry = Registry::load(
        Some(&carbon_core::extension::ExtensionDirs::new(
            temp.path().join("prefs"),
        )),
        &state,
    );
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    let loaded = registry.get(&ExtensionId::new("acme.hello")).unwrap();

    let runtime = WasmRuntime::new().unwrap();
    let context = Arc::new(HostContext::default());
    *context.workspace.lock().unwrap() = Some(temp.path().to_path_buf());
    context
        .settings
        .lock()
        .unwrap()
        .insert("acme.hello.greeting".into(), "\"hey\"".into());

    // Nothing granted: init runs, but nothing outside the sandbox works.
    let started = std::time::Instant::now();
    let bare = runtime
        .load(loaded, BTreeSet::new(), context.clone())
        .unwrap();
    println!("load: {:?}", started.elapsed());
    assert!(
        matches!(&events(&context)[..], [HostEvent::Log { message, .. }] if message == "hello: started")
    );
    for command in ["read-package", "run-git", "fetch"] {
        let err = bare.run_command(command).unwrap_err().to_string();
        assert!(err.contains("not allowed"), "{command}: {err}");
    }

    // Granted: the package file reads, but nothing outside the package.
    let grants: BTreeSet<String> = ["fs.read:package", "process.exec:git"]
        .map(String::from)
        .into();
    let hello = runtime.load(loaded, grants, context.clone()).unwrap();
    events(&context);
    hello.run_command("read-package").unwrap();
    assert!(
        matches!(&events(&context)[..], [HostEvent::Log { message, .. }] if message.contains("acme.hello"))
    );
    assert!(hello.run_command("read-outside").is_err());
    assert!(
        hello
            .run_command("fetch")
            .unwrap_err()
            .to_string()
            .contains("net:example.com")
    );
    if carbon_core::lsp::find_program("git").is_some() {
        hello.run_command("run-git").unwrap();
    }
    events(&context);

    // Commands, settings, the open folder, views, language servers.
    hello.run_command("count").unwrap();
    let notified = events(&context);
    let [HostEvent::Notify { message, .. }] = &notified[..] else {
        panic!("{notified:?}")
    };
    assert!(
        message.starts_with("\"hey\" from ") && message.ends_with("(run 1)"),
        "{message}"
    );
    let view = hello.render_view("runs").unwrap();
    assert_eq!(view.len(), 3);
    assert_eq!(
        view[1],
        Widget::Text {
            text: "Ran 1 times".into(),
            muted: false
        }
    );
    let command = hello.language_server_command("echo").unwrap().unwrap();
    assert_eq!(
        (command.program.as_str(), command.args.len()),
        ("echo-ls", 1)
    );
    assert!(hello.language_server_command("other").unwrap().is_none());
    assert_eq!(
        hello.run_command("nope").unwrap_err(),
        CallError::Returned("unknown command nope".into())
    );

    // API 0.2: storage survives reloading the code; writing needs its grant;
    // a timer comes back as an event.
    *context.storage.lock().unwrap() = Some(temp.path().join("storage"));
    hello.run_command("remember").unwrap();
    let again = runtime
        .load(
            loaded,
            ["fs.write:workspace".to_string()].into(),
            context.clone(),
        )
        .unwrap();
    events(&context);
    again.run_command("remember").unwrap();
    assert!(
        matches!(&events(&context)[..], [HostEvent::Notify { message, .. }] if message == "seen 2")
    );
    assert!(
        hello
            .run_command("write")
            .unwrap_err()
            .to_string()
            .contains("fs.write:workspace")
    );
    again.run_command("write").unwrap();
    assert_eq!(
        fs::read_to_string(temp.path().join("out/hello.txt")).unwrap(),
        "hello"
    );
    again.run_command("timer").unwrap();
    assert!(matches!(
        &events(&context)[..],
        [HostEvent::Timer { tag, after_ms: 10, .. }] if tag == "tick"
    ));
    again
        .on_event(&ExtensionEvent::TimerFired("tick".into()))
        .unwrap();
    assert!(
        matches!(&events(&context)[..], [HostEvent::Notify { message, .. }] if message == "timer tick")
    );
    let doc = carbon_core::graph::GraphDoc::new("acme.hello/x", "X");
    assert!(
        again
            .check_graph("x.yaml", &doc)
            .unwrap()
            .contains("0 nodes")
    );

    // A runaway call stops at the limit; a panic is an error, not a crash.
    let started = std::time::Instant::now();
    let err = bare.run_command("spin").unwrap_err();
    assert!(
        matches!(&err, CallError::Fault(why) if why.contains("longer than")),
        "{err}"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(8));
    let err = hello.run_command("panic").unwrap_err();
    assert!(
        matches!(&err, CallError::Fault(why) if why.contains("failed")),
        "{err}"
    );
}
