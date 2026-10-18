//! Terminal sessions: a shell on a PTY with its emulator. Windows ConPTY asks
//! for the cursor position before it runs anything; the emulator answers.

use std::time::{Duration, Instant};

use carbon_core::terminal::alacritty_terminal::grid::Dimensions;
use carbon_core::terminal::shells;
use carbon_core::{Core, TerminalEvent, TerminalId, TerminalSpec};

/// A shell that reads commands from input, plus the line break it expects.
fn shell() -> (TerminalSpec, &'static str) {
    let (program, newline) = if cfg!(windows) {
        ("cmd.exe", "\r\n")
    } else {
        ("sh", "\n")
    };
    let spec = TerminalSpec {
        program: Some(program.into()),
        ..TerminalSpec::default()
    };
    (spec, newline)
}

/// Waits until `done` holds for the terminal (checked on every event), or
/// times out. Returns whether it held.
fn wait_for(core: &Core, id: TerminalId, mut done: impl FnMut(&Core) -> bool) -> bool {
    let events = core.terminals.get(id).unwrap().events();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if done(core) {
            return true;
        }
        match events.try_recv() {
            Ok(TerminalEvent::Wakeup) => core.terminals.get(id).unwrap().ack_wakeup(),
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    done(core)
}

#[test]
fn runs_input_and_shows_output_on_screen() {
    let dir = tempfile::tempdir().unwrap();
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();

    let (spec, newline) = shell();
    let id = core.spawn_terminal(spec).unwrap();
    let terminal = core.terminals.get(id).unwrap();
    terminal.resize(100, 30).unwrap();
    terminal
        .write(format!("echo carbon-pty-ok{newline}").as_bytes())
        .unwrap();

    // The echo's output line (not just the typed command) shows up.
    let shown = wait_for(&core, id, |core| {
        let screen = core.terminals.get(id).unwrap().screen_text();
        screen.lines().any(|line| line.trim() == "carbon-pty-ok")
    });
    let terminal = core.terminals.get(id).unwrap();
    assert!(shown, "screen was:\n{}", terminal.screen_text());
    let term = terminal.term().lock();
    assert_eq!((term.columns(), term.screen_lines()), (100, 30));
    drop(term);

    terminal.write(format!("exit{newline}").as_bytes()).unwrap();
    let exited = wait_for(&core, id, |core| {
        core.terminals.get(id).unwrap().exit_status().is_some()
    });
    assert!(exited);
    assert_eq!(core.terminals.get(id).unwrap().exit_status(), Some(Some(0)));
}

#[test]
fn close_kills_a_running_terminal() {
    let mut core = Core::new();
    let (spec, _) = shell();
    let id = core.spawn_terminal(spec).unwrap();
    let events = core.terminals.get(id).unwrap().events();

    core.terminals.close(id).unwrap();
    assert!(core.terminals.get(id).is_none());
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut exited = false;
    while Instant::now() < deadline && !exited {
        exited = matches!(
            events.recv_blocking(),
            Ok(TerminalEvent::Exited(_)) | Err(_)
        );
    }
    assert!(exited, "killed shell should report exit");
}

#[test]
fn a_default_shell_is_found() {
    let profiles = shells::profiles();
    assert!(!profiles.is_empty());
    let default = shells::default_profile().unwrap();
    assert_eq!(default, profiles[0]);
    if cfg!(windows) {
        assert!(
            ["PowerShell", "Windows PowerShell"].contains(&default.name.as_str()),
            "default was {}",
            default.name
        );
    }
    assert_eq!(shells::profile_named(&default.name), Some(default));
}

/// Real PowerShell with Carbon's integration script: commands, exit codes and
/// the working directory come back. Run with `--ignored` (starts PowerShell).
#[test]
#[ignore]
fn powershell_integration_reports_commands() {
    let dir = tempfile::tempdir().unwrap();
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();
    let profile = shells::default_profile().unwrap();
    let cwd = dunce::canonicalize(dir.path()).unwrap();
    let id = core
        .spawn_terminal(profile.spec(Some(cwd.clone()), 120, 30))
        .unwrap();
    let terminal = core.terminals.get(id).unwrap();
    // Wait for the first prompt before typing.
    assert!(wait_for(&core, id, |core| !core
        .terminals
        .get(id)
        .unwrap()
        .commands()
        .is_empty()));
    terminal.write(b"echo carbon-si-ok\r").unwrap();
    let finished = wait_for(&core, id, |core| {
        core.terminals
            .get(id)
            .unwrap()
            .commands()
            .iter()
            .any(|command| command.finished)
    });
    let terminal = core.terminals.get(id).unwrap();
    let commands = terminal.commands();
    assert!(
        finished,
        "commands: {commands:?}\nscreen:\n{}",
        terminal.screen_text()
    );
    let first = commands.iter().find(|command| command.finished).unwrap();
    assert_eq!(first.line.as_deref(), Some("echo carbon-si-ok"));
    assert_eq!(first.exit_code, Some(0));
    assert!(
        first
            .output
            .as_deref()
            .unwrap_or("")
            .contains("carbon-si-ok"),
        "{first:?}"
    );
    assert_eq!(terminal.cwd(), Some(cwd));
}
