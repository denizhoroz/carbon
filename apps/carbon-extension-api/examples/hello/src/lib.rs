//! A small example extension: a command that counts the files in the open
//! folder's top level and a view that shows how often it ran. It also tries
//! things it was not granted, so the host's checks can be seen.

use std::sync::atomic::{AtomicU32, Ordering};

use carbon_extension_api::{
    Extension, ServerCommand, http_get, log, notify, read_file, register_extension, run_process,
    setting, workspace_root,
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
}

register_extension!(Hello);
