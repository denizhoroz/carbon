//! PTY sessions. The core owns the process and raw bytes; turning bytes into a
//! screen (VT parsing, grid, scrollback) is the terminal panel's job and is not
//! decided yet (`project/not_built/README.md`).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::id::define_id;
use crate::{CoreError, Result};

define_id!(
    /// A running (or finished) terminal session.
    TerminalId
);

/// What to run. `program: None` starts the user's default shell
/// (`$SHELL` on Unix, `%COMSPEC%` on Windows).
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
    /// Raw bytes from the PTY (text plus escape sequences).
    Output(Vec<u8>),
    /// The process ended; `None` when the exit code is unknown.
    Exited(Option<u32>),
}

pub struct Terminal {
    id: TerminalId,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    events: async_channel::Receiver<TerminalEvent>,
}

fn pty_error(err: impl std::fmt::Display) -> CoreError {
    CoreError::Terminal(err.to_string())
}

impl Terminal {
    fn spawn(id: TerminalId, spec: TerminalSpec) -> Result<Self> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
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
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let mut child = pair.slave.spawn_command(command).map_err(pty_error)?;
        // The child holds its own handle; dropping ours lets EOF reach the reader.
        drop(pair.slave);

        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().map_err(pty_error)?;
        let writer = pair.master.take_writer().map_err(pty_error)?;
        let (sender, events) = async_channel::unbounded();

        let output = sender.clone();
        std::thread::Builder::new()
            .name(format!("terminal-{id}-read"))
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if output
                                .send_blocking(TerminalEvent::Output(buf[..n].to_vec()))
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(pty_error)?;

        std::thread::Builder::new()
            .name(format!("terminal-{id}-wait"))
            .spawn(move || {
                let code = child.wait().ok().map(|status| status.exit_code());
                let _ = sender.send_blocking(TerminalEvent::Exited(code));
            })
            .map_err(pty_error)?;

        Ok(Self {
            id,
            master: pair.master,
            writer,
            killer,
            events,
        })
    }

    pub fn id(&self) -> TerminalId {
        self.id
    }

    /// Output and exit events. Clones share one stream (each event goes to one receiver).
    pub fn events(&self) -> async_channel::Receiver<TerminalEvent> {
        self.events.clone()
    }

    /// Sends keyboard input (bytes, e.g. `b"ls\r"`).
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer
            .write_all(bytes)
            .and_then(|_| self.writer.flush())
            .map_err(pty_error)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(pty_error)
    }

    pub fn kill(&mut self) -> Result<()> {
        self.killer.kill().map_err(pty_error)
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
