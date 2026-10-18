//! The emulator side of a terminal session: the alacritty `Term` (screen,
//! scrollback, modes), fed on the PTY reader thread, and what it asks of the
//! outside world (answers written back to the shell, title changes, bells).

use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::vte::ansi::Processor;

use super::osc::{Mark, Piece, Sniffer};
use super::{Command, Term, TerminalEvent};

/// Rows of scrollback kept per terminal.
pub const SCROLLBACK_LINES: usize = 10_000;
/// Most output text kept per command (agents read it), from the end.
const MAX_COMMAND_OUTPUT: usize = 256 * 1024;
/// Commands remembered per terminal (oldest dropped).
const MAX_COMMANDS: usize = 1000;

/// Grid size in cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u16,
    pub rows: u16,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.rows.max(1) as usize
    }

    fn columns(&self) -> usize {
        self.cols.max(1) as usize
    }
}

/// What the reader thread, the listener and the owner share.
#[derive(Default)]
pub(crate) struct Session {
    pub title: Option<String>,
    pub cwd: Option<PathBuf>,
    pub commands: Vec<Command>,
    pub exited: Option<Option<u32>>,
    pub size: Option<TermSize>,
    /// Cell size in pixels, for programs that ask for the window size.
    pub cell_px: (u16, u16),
}

/// Receives the emulator's requests (it runs inside `Processor::advance`, on
/// the reader thread, with the `Term` locked: so it only queues work).
pub struct Listener {
    pub(crate) input: async_channel::Sender<Vec<u8>>,
    pub(crate) events: async_channel::Sender<TerminalEvent>,
    pub(crate) session: Arc<Mutex<Session>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            // Answers to the shell's questions, e.g. the cursor position
            // ConPTY asks for before it starts anything.
            Event::PtyWrite(text) => {
                _ = self.input.try_send(text.into_bytes());
            }
            Event::Title(title) => {
                self.session.lock().unwrap().title = Some(title.clone());
                _ = self.events.try_send(TerminalEvent::Title(title));
            }
            Event::ResetTitle => {
                self.session.lock().unwrap().title = None;
                _ = self.events.try_send(TerminalEvent::Title(String::new()));
            }
            Event::Bell => {
                _ = self.events.try_send(TerminalEvent::Bell);
            }
            Event::ClipboardStore(_, text) => {
                _ = self.events.try_send(TerminalEvent::Clipboard(text));
            }
            Event::TextAreaSizeRequest(format) => {
                let session = self.session.lock().unwrap();
                if let Some(size) = session.size {
                    let (cell_width, cell_height) = session.cell_px;
                    let reply = format(WindowSize {
                        num_lines: size.rows,
                        num_cols: size.cols,
                        cell_width,
                        cell_height,
                    });
                    _ = self.input.try_send(reply.into_bytes());
                }
            }
            _ => {}
        }
    }
}

/// The cursor's line counted from the top of the scrollback, so it stays put
/// while output scrolls (until the scrollback is full and old lines drop).
fn absolute_line(term: &Term) -> i64 {
    term.grid().history_size() as i64 + term.grid().cursor.point.line.0 as i64
}

/// An absolute line back to a grid line, if it is still in the scrollback.
pub(crate) fn grid_line(term: &Term, absolute: i64) -> Option<Line> {
    let line = absolute - term.grid().history_size() as i64;
    let top = -(term.grid().history_size() as i64);
    let bottom = term.screen_lines() as i64 - 1;
    (top..=bottom).contains(&line).then_some(Line(line as i32))
}

fn apply_mark(term: &Term, session: &mut Session, mark: Mark) {
    let line = absolute_line(term);
    match mark {
        Mark::PromptStart => {
            let cwd = session.cwd.clone();
            if session.commands.len() >= MAX_COMMANDS {
                session.commands.remove(0);
            }
            session.commands.push(Command {
                prompt_line: line,
                cwd,
                ..Command::default()
            });
        }
        Mark::CommandStart => {}
        Mark::CommandLine(text) => {
            if let Some(command) = session.commands.last_mut() {
                command.line = Some(text);
            }
        }
        Mark::CommandExecuted => {
            // Output starts on the next line when the shell marks this before
            // the line break (PowerShell's Enter handler does).
            let next = term.grid().cursor.point.column.0 > 0;
            if let Some(command) = session.commands.last_mut() {
                command.output_line = Some(line + next as i64);
            }
        }
        Mark::CommandFinished(code) => {
            let Some(command) = session.commands.last_mut() else {
                return;
            };
            command.exit_code = code;
            command.finished = true;
            if let Some(start) = command.output_line.and_then(|start| grid_line(term, start)) {
                let end = term.grid().cursor.point.line;
                if end > start {
                    let mut text = term.bounds_to_string(
                        Point::new(start, Column(0)),
                        Point::new(end - 1, term.last_column()),
                    );
                    if text.len() > MAX_COMMAND_OUTPUT {
                        let cut = text.len() - MAX_COMMAND_OUTPUT;
                        let cut = (cut..text.len())
                            .find(|&i| text.is_char_boundary(i))
                            .unwrap_or(text.len());
                        text.drain(..cut);
                    }
                    command.output = Some(text.trim_end().to_string());
                }
            }
        }
        Mark::Cwd(path) => {
            let path = PathBuf::from(path);
            if let Some(command) = session.commands.last_mut() {
                command.cwd = Some(path.clone());
            }
            session.cwd = Some(path);
        }
    }
}

/// Reads the PTY until it closes: parses into the `Term` (shell integration
/// marks go to the session) and tells the owner once per burst that the
/// screen changed.
pub(crate) fn read_loop(
    mut reader: Box<dyn Read + Send>,
    term: Arc<FairMutex<Term>>,
    session: Arc<Mutex<Session>>,
    wakeup: Arc<AtomicBool>,
    events: async_channel::Sender<TerminalEvent>,
) {
    let mut processor: Processor = Processor::new();
    let mut sniffer = Sniffer::default();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        {
            let mut term = term.lock();
            for piece in sniffer.feed(&buf[..n]) {
                match piece {
                    Piece::Bytes(bytes) => processor.advance(&mut *term, &bytes),
                    Piece::Mark(mark) => apply_mark(&term, &mut session.lock().unwrap(), mark),
                }
            }
        }
        // One wakeup until the owner has looked (`Terminal::ack_wakeup`).
        if !wakeup.swap(true, Ordering::AcqRel)
            && events.send_blocking(TerminalEvent::Wakeup).is_err()
        {
            break;
        }
    }
}
