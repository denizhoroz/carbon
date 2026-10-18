//! Terminal sessions: a shell on a PTY plus its emulator (screen, scrollback,
//! modes) and what shell integration reports (commands, exit codes, cwd).
//!
//! The emulator lives here, not in the UI, so agents and a later daemon can
//! read screens and run commands without a window. A UI locks [`Terminal::term`]
//! to draw it and sends input through [`Terminal::write`].

mod emulator;
pub mod osc;
pub mod shells;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub use alacritty_terminal;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, TermMode};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::id::define_id;
use crate::{CoreError, Result};
pub use emulator::{Listener, SCROLLBACK_LINES, TermSize};
use emulator::{Session, read_loop};

define_id!(
    /// A running (or finished) terminal session.
    TerminalId
);

/// The emulator state of one terminal.
pub type Term = alacritty_terminal::Term<Listener>;

/// What to run. `program: None` starts the user's default shell
/// (`$SHELL` on Unix, `%COMSPEC%` on Windows); see [`shells`] for profiles.
#[derive(Clone, Debug)]
pub struct TerminalSpec {
    pub program: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalSpec {
    fn default() -> Self {
        Self {
            program: None,
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            cols: 80,
            rows: 24,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    /// The screen changed. Sent once until [`Terminal::ack_wakeup`].
    Wakeup,
    /// The shell set its title (empty: back to the default).
    Title(String),
    Bell,
    /// The shell asked to copy text (OSC 52).
    Clipboard(String),
    /// The process ended; `None` when the exit code is unknown.
    Exited(Option<u32>),
}

/// One command the shell reported through shell integration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Command {
    /// The command line as typed, when the shell sends it.
    pub line: Option<String>,
    pub cwd: Option<PathBuf>,
    pub exit_code: Option<i32>,
    pub finished: bool,
    /// Output text (last 256 KB), once finished.
    pub output: Option<String>,
    /// Where the prompt starts and the output starts, counted from the top
    /// of the scrollback (see [`Terminal::grid_line`]).
    pub prompt_line: i64,
    pub output_line: Option<i64>,
}

/// Where a command's prompt is and how it ended: for drawing marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandMark {
    pub prompt_line: i64,
    pub exit_code: Option<i32>,
    pub finished: bool,
}

pub struct Terminal {
    id: TerminalId,
    master: Box<dyn MasterPty + Send>,
    input: async_channel::Sender<Vec<u8>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    term: Arc<FairMutex<Term>>,
    session: Arc<Mutex<Session>>,
    wakeup: Arc<AtomicBool>,
    events: async_channel::Receiver<TerminalEvent>,
}

fn pty_error(err: impl std::fmt::Display) -> CoreError {
    CoreError::Terminal(err.to_string())
}

impl Terminal {
    fn spawn(id: TerminalId, spec: TerminalSpec) -> Result<Self> {
        let size = TermSize {
            cols: spec.cols.max(2),
            rows: spec.rows.max(1),
        };
        let pair = native_pty_system()
            .openpty(pty_size(size))
            .map_err(pty_error)?;

        let mut command = match &spec.program {
            Some(program) => {
                let mut command = CommandBuilder::new(program);
                command.args(&spec.args);
                command
            }
            None => CommandBuilder::new_default_prog(),
        };
        if let Some(cwd) = &spec.cwd {
            command.cwd(cwd);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let mut child = pair.slave.spawn_command(command).map_err(pty_error)?;
        // The child holds its own handle; dropping ours lets EOF reach the reader.
        drop(pair.slave);

        let killer = child.clone_killer();
        let reader = pair.master.try_clone_reader().map_err(pty_error)?;
        let mut writer = pair.master.take_writer().map_err(pty_error)?;
        let (sender, events) = async_channel::unbounded();
        let (input, input_rx) = async_channel::unbounded::<Vec<u8>>();
        let session = Arc::new(Mutex::new(Session {
            cwd: spec.cwd.clone(),
            size: Some(size),
            ..Session::default()
        }));
        let listener = Listener {
            input: input.clone(),
            events: sender.clone(),
            session: session.clone(),
        };
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            ..Config::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, listener)));
        let wakeup = Arc::new(AtomicBool::new(false));

        // Input goes through its own thread, so neither the UI nor the
        // emulator's answers ever wait on a full PTY pipe.
        std::thread::Builder::new()
            .name(format!("terminal-{id}-write"))
            .spawn(move || {
                while let Ok(bytes) = input_rx.recv_blocking() {
                    if writer
                        .write_all(&bytes)
                        .and_then(|_| writer.flush())
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(pty_error)?;

        {
            let (term, session, wakeup, sender) = (
                term.clone(),
                session.clone(),
                wakeup.clone(),
                sender.clone(),
            );
            std::thread::Builder::new()
                .name(format!("terminal-{id}-read"))
                .spawn(move || read_loop(reader, term, session, wakeup, sender))
                .map_err(pty_error)?;
        }

        {
            let session = session.clone();
            std::thread::Builder::new()
                .name(format!("terminal-{id}-wait"))
                .spawn(move || {
                    let code = child.wait().ok().map(|status| status.exit_code());
                    session.lock().unwrap().exited = Some(code);
                    let _ = sender.send_blocking(TerminalEvent::Exited(code));
                })
                .map_err(pty_error)?;
        }

        Ok(Self {
            id,
            master: pair.master,
            input,
            killer,
            term,
            session,
            wakeup,
            events,
        })
    }

    pub fn id(&self) -> TerminalId {
        self.id
    }

    /// Screen changes, title, bell, clipboard, exit. Clones share one stream
    /// (each event goes to one receiver).
    pub fn events(&self) -> async_channel::Receiver<TerminalEvent> {
        self.events.clone()
    }

    /// The emulator. Lock it briefly (the reader thread parses under it).
    pub fn term(&self) -> &Arc<FairMutex<Term>> {
        &self.term
    }

    /// Call after handling [`TerminalEvent::Wakeup`], before reading the
    /// screen, so the next change sends a new one.
    pub fn ack_wakeup(&self) {
        self.wakeup.store(false, Ordering::Release);
    }

    /// Sends input bytes (keys, e.g. `b"ls\r"`). Never blocks.
    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        self.input
            .try_send(bytes.to_vec())
            .map_err(|_| CoreError::Terminal("terminal input closed".into()))
    }

    /// Pastes text: wrapped in bracketed-paste markers when the program asked
    /// for them, otherwise with line breaks sent as Enter.
    pub fn paste(&self, text: &str) -> Result<()> {
        let bracketed = self.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let bytes = if bracketed {
            // A pasted end marker would end the paste early.
            let text = text.replace('\x1b', "");
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r")
        };
        self.scroll(Scroll::Bottom);
        self.write(bytes.as_bytes())
    }

    /// Resizes the PTY and the screen (cell size in pixels is for programs
    /// that ask; 0 when unknown).
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.resize_cells(cols, rows, 0, 0)
    }

    pub fn resize_cells(
        &self,
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<()> {
        let size = TermSize {
            cols: cols.max(2),
            rows: rows.max(1),
        };
        {
            let mut session = self.session.lock().unwrap();
            if session.size == Some(size) {
                return Ok(());
            }
            session.size = Some(size);
            session.cell_px = (cell_width, cell_height);
        }
        let pulled = {
            let mut term = self.term.lock();
            let before = term.grid().history_size() as i64;
            term.resize(size);
            before - term.grid().history_size() as i64
        };
        // Growing pulls scrollback lines into view, but ConPTY repaints its
        // own viewport from the top, so on Windows the content (and the
        // command marks with it) ends up that many lines higher.
        if cfg!(windows) && pulled > 0 {
            let mut session = self.session.lock().unwrap();
            for command in &mut session.commands {
                command.prompt_line -= pulled;
                if let Some(line) = &mut command.output_line {
                    *line -= pulled;
                }
            }
        }
        self.master.resize(pty_size(size)).map_err(pty_error)
    }

    pub fn size(&self) -> TermSize {
        self.session
            .lock()
            .unwrap()
            .size
            .unwrap_or(TermSize { cols: 80, rows: 24 })
    }

    pub fn scroll(&self, scroll: Scroll) {
        self.term.lock().scroll_display(scroll);
    }

    /// Starts a selection at a grid point (`Simple`, `Semantic` = word,
    /// `Lines`).
    pub fn start_selection(&self, ty: SelectionType, point: Point, side: Side) {
        self.term.lock().selection = Some(Selection::new(ty, point, side));
    }

    pub fn update_selection(&self, point: Point, side: Side) {
        if let Some(selection) = self.term.lock().selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term
            .lock()
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }

    /// The visible screen as text, one line per row (trailing spaces cut).
    pub fn screen_text(&self) -> String {
        let term = self.term.lock();
        let offset = term.grid().display_offset() as i32;
        let rows = term.screen_lines() as i32;
        (0..rows)
            .map(|row| {
                let line = Line(row - offset);
                term.bounds_to_string(
                    Point::new(line, Column(0)),
                    Point::new(line, term.last_column()),
                )
                .trim_end()
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The title the shell set, if any.
    pub fn title(&self) -> Option<String> {
        self.session.lock().unwrap().title.clone()
    }

    /// The shell's working directory: reported by shell integration, else
    /// where it started.
    pub fn cwd(&self) -> Option<PathBuf> {
        self.session.lock().unwrap().cwd.clone()
    }

    /// Commands reported by shell integration, oldest first.
    pub fn commands(&self) -> Vec<Command> {
        self.session.lock().unwrap().commands.clone()
    }

    /// Prompt positions and results of the commands, cheap to read every frame.
    pub fn command_marks(&self) -> Vec<CommandMark> {
        let session = self.session.lock().unwrap();
        session
            .commands
            .iter()
            .map(|command| CommandMark {
                prompt_line: command.prompt_line,
                exit_code: command.exit_code,
                finished: command.finished,
            })
            .collect()
    }

    /// `Some(code)` once the process ended.
    pub fn exit_status(&self) -> Option<Option<u32>> {
        self.session.lock().unwrap().exited
    }

    /// A line from [`Command`] in grid coordinates, if still in the scrollback.
    pub fn grid_line(term: &Term, absolute: i64) -> Option<Line> {
        emulator::grid_line(term, absolute)
    }

    pub fn kill(&mut self) -> Result<()> {
        self.killer.kill().map_err(pty_error)
    }
}

fn pty_size(size: TermSize) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// All terminal sessions.
#[derive(Default)]
pub struct TerminalStore {
    next_id: u64,
    terminals: HashMap<TerminalId, Terminal>,
}

impl TerminalStore {
    pub fn spawn(&mut self, spec: TerminalSpec) -> Result<TerminalId> {
        self.next_id += 1;
        let id = TerminalId(self.next_id);
        self.terminals.insert(id, Terminal::spawn(id, spec)?);
        Ok(id)
    }

    pub fn get(&self, id: TerminalId) -> Option<&Terminal> {
        self.terminals.get(&id)
    }

    pub fn get_mut(&mut self, id: TerminalId) -> Option<&mut Terminal> {
        self.terminals.get_mut(&id)
    }

    pub fn ids(&self) -> impl Iterator<Item = TerminalId> + '_ {
        self.terminals.keys().copied()
    }

    /// Kills the process (if still running) and drops the session.
    pub fn close(&mut self, id: TerminalId) -> Result<()> {
        let mut terminal = self
            .terminals
            .remove(&id)
            .ok_or(CoreError::NotFound("terminal"))?;
        // Already exited is fine.
        let _ = terminal.kill();
        Ok(())
    }
}
