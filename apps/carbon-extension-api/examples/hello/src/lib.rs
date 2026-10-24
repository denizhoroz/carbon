//! A small example extension: a command that counts the files in the open
//! folder's top level and a view that shows how often it ran. It also tries
//! things it was not granted, so the host's checks can be seen, and (API
//! 0.2) keeps a value in storage, writes a file, and answers a timer.

use std::sync::atomic::{AtomicU32, Ordering};

use carbon_extension_api::{
    Event, Extension, ServerCommand, graphs, http_get, log, notify, read_file, register_extension,
    run_process, set_timer, setting, storage_get, storage_set, workspace_root, write_file,
};

static RUNS: AtomicU32 = AtomicU32::new(0);

struct Hello;

impl Extension for Hello {
    fn init() -> Result<(), String> {
        log("hello: started");
        Ok(())
    }

    fn run_command(id: String) -> Result<(), String> {
        match id.as_str() {
            "count" => {
                let runs = RUNS.fetch_add(1, Ordering::Relaxed) + 1;
                let root = workspace_root().unwrap_or_else(|| "(no folder)".into());
                let greeting = setting("greeting").unwrap_or_else(|| "\"hi\"".into());
                notify(&format!("{greeting} from {root} (run {runs})"));
                Ok(())
            }
            "read-package" => read_file("extension.toml").map(|text| log(&text)),
            "read-outside" => read_file("../secret.txt").map(|_| ()),
            "run-git" => run_process("git", &["--version".to_string()]).map(|out| log(&out.stdout)),
            "fetch" => http_get("https://example.com/").map(|_| ()),
            "remember" => {
                let seen: u32 = storage_get("seen").and_then(|v| v.parse().ok()).unwrap_or(0);
                storage_set("seen", &(seen + 1).to_string())?;
                notify(&format!("seen {}", seen + 1));
                Ok(())
            }
            "write" => write_file("out/hello.txt", "hello"),
            "timer" => {
                set_timer(10, "tick");
                Ok(())
            }
            "spin" => loop {
                std::hint::spin_loop();
            },
            "panic" => panic!("on purpose"),
            other => Err(format!("unknown command {other}")),
        }
    }

    fn language_server_command(server_id: String) -> Result<Option<ServerCommand>, String> {
        Ok((server_id == "echo").then(|| ServerCommand {
            program: "echo-ls".into(),
            args: vec!["--stdio".into()],
            env: vec![("ECHO".into(), "1".into())],
        }))
    }

    fn render_view(view: String) -> Result<String, String> {
        let runs = RUNS.load(Ordering::Relaxed);
        Ok(format!(
            r#"[{{"type":"heading","text":"{view}"}},{{"type":"text","text":"Ran {runs} times"}},{{"type":"button","label":"Count","command":"count"}}]"#
        ))
    }

    fn on_event(event: Event) -> Result<(), String> {
        match event {
            Event::TimerFired(tag) => {
                notify(&format!("timer {tag}"));
                Ok(())
            }
            other => Err(format!("unexpected {other:?}")),
        }
    }

    fn check_graph(doc: graphs::GraphDoc) -> Result<String, String> {
        Ok(format!(
            r#"[{{"severity":"info","message":"{} nodes"}}]"#,
            doc.nodes.len()
        ))
    }
}

register_extension!(Hello);
