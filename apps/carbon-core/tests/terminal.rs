use std::time::{Duration, Instant};

use carbon_core::{Core, Terminal, TerminalEvent, TerminalSpec};

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

/// Device Status Report: "where is the cursor?". Windows ConPTY sends it first and
/// waits for the answer before running anything; a terminal emulator replies
/// automatically. The terminal panel must do the same.
const CURSOR_POSITION_REQUEST: &str = "[6n";

/// Collects output until the process exits (plus a short drain), or times out.
/// With a terminal, answers cursor-position requests like an emulator would.
fn run_until_exit(
    mut terminal: Option<&mut Terminal>,
    events: &async_channel::Receiver<TerminalEvent>,
) -> (String, Option<Option<u32>>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut output = Vec::new();
    let mut exited = None;
    let mut drain_until = None;
    while Instant::now() < deadline && drain_until.is_none_or(|t| Instant::now() < t) {
        match events.try_recv() {
            Ok(TerminalEvent::Output(bytes)) => {
                let requests = String::from_utf8_lossy(&bytes)
                    .matches(CURSOR_POSITION_REQUEST)
                    .count();
                if let Some(terminal) = terminal.as_deref_mut() {
                    for _ in 0..requests {
                        terminal.write(b"[1;1R").unwrap();
                    }
                }
                output.extend(bytes);
            }
            Ok(TerminalEvent::Exited(code)) => {
                exited = Some(code);
                drain_until = Some(Instant::now() + Duration::from_millis(500));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    (String::from_utf8_lossy(&output).into_owned(), exited)
}

#[test]
fn runs_input_and_reports_output_and_exit() {
    let dir = tempfile::tempdir().unwrap();
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();

    let (spec, newline) = shell();
    let id = core.spawn_terminal(spec).unwrap();
    let terminal = core.terminals.get_mut(id).unwrap();
    let events = terminal.events();
    terminal.resize(100, 30).unwrap();
    terminal
        .write(format!("echo carbon-pty-ok{newline}exit{newline}").as_bytes())
        .unwrap();

    let (output, exited) = run_until_exit(Some(terminal), &events);
    assert!(output.contains("carbon-pty-ok"), "output was: {output:?}");
    assert_eq!(exited, Some(Some(0)), "output was: {output:?}");
}

#[test]
fn close_kills_a_running_terminal() {
    let mut core = Core::new();
    let (spec, _) = shell();
    let id = core.spawn_terminal(spec).unwrap();
    let events = core.terminals.get(id).unwrap().events();

    core.terminals.close(id).unwrap();
    assert!(core.terminals.get(id).is_none());
    let (_, exited) = run_until_exit(None, &events);
    assert!(exited.is_some(), "killed shell should report exit");
}
