//! One ACP session: its thread starts the program, sets up the session, runs
//! prompts one at a time, and turns what the agent sends into `AcpEvent`s.
//! Everything reaches the thread through one channel (`Msg`): commands from
//! the manager and every line the agent writes.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::resolve::quiet;
use super::transcript::Transcript;
use super::{
    AcpEvent, AgentProgram, FailureKind, PermissionMode, PermissionOption, SessionId,
    TranscriptItem, Usage,
};

/// Longest wait for the agent to start and set up a session (an npx first
/// run downloads the adapter).
const SETUP_TIMEOUT: Duration = Duration::from_secs(180);
/// Largest answer passed on whole.
const MAX_TEXT: usize = 256 * 1024;
/// Lines of the agent's stderr kept for error messages.
const STDERR_LINES: usize = 20;

pub(super) enum Msg {
    Prompt(String),
    Cancel,
    Close,
    Answer {
        request: u64,
        option: Option<String>,
    },
    Incoming(Value),
    Exited,
}

pub(super) struct Setup {
    pub id: SessionId,
    pub owner: String,
    pub program: AgentProgram,
    pub cwd: PathBuf,
    pub mode: PermissionMode,
    pub tools: Option<PathBuf>,
}

/// Why the session ended.
enum End {
    Closed,
    Failed(FailureKind, String),
}

struct Session<'a> {
    setup: &'a Setup,
    rx: &'a Receiver<Msg>,
    events: &'a async_channel::Sender<AcpEvent>,
    transcript: &'a Mutex<Transcript>,
    stdin: ChildStdin,
    stderr: Arc<Mutex<VecDeque<String>>>,
    next_id: i64,
    session_id: String,
    /// Prompts waiting for the one running.
    queue: VecDeque<String>,
    /// The running prompt's request id, its text so far, the message the
    /// last chunk belonged to.
    prompting: Option<(i64, String, Option<String>)>,
    /// Permission requests waiting for an answer: our number -> the
    /// agent's request id and its title.
    permissions: HashMap<u64, (Value, String)>,
    next_permission: u64,
    cost_usd: Option<f64>,
}

/// Runs the session to its end; returns the last event (`Failed` or
/// `Closed`) for the caller to send once it has noted the end.
pub(super) fn run(
    setup: Setup,
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    events: async_channel::Sender<AcpEvent>,
    transcript: Arc<Mutex<Transcript>>,
) -> AcpEvent {
    let owner = setup.owner.clone();
    let id = setup.id;
    let emit = |event: AcpEvent| {
        _ = events.send_blocking(event);
    };
    let note = |text: String| {
        if let Ok(mut t) = transcript.lock() {
            t.push(TranscriptItem::Note(text));
        }
    };
    emit(AcpEvent::Starting {
        session: id,
        owner: owner.clone(),
        message: format!("Starting {}…", setup.program.name),
    });
    let status = |message: String| {
        emit(AcpEvent::Starting {
            session: id,
            owner: owner.clone(),
            message,
        })
    };
    let launch = match setup.program.resolve(setup.tools.as_deref(), &status) {
        Ok(launch) => launch,
        Err(err) => {
            note(err.clone());
            return AcpEvent::Failed {
                session: id,
                owner,
                kind: FailureKind::MissingProgram,
                message: err,
            };
        }
    };
    let mut command = quiet(&launch.program);
    command
        .args(&launch.args)
        .envs(launch.env.iter().cloned())
        .current_dir(&setup.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            let message = format!("cannot start {}: {err}", launch.program.display());
            note(message.clone());
            return AcpEvent::Failed {
                session: id,
                owner,
                kind: FailureKind::MissingProgram,
                message,
            };
        }
    };
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr_pipe = child.stderr.take().expect("piped stderr");
    let stderr: Arc<Mutex<VecDeque<String>>> = Arc::default();
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && tx.send(Msg::Incoming(value)).is_err()
                {
                    return;
                }
            }
            _ = tx.send(Msg::Exited);
        });
        let stderr = stderr.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr_pipe).lines() {
                let Ok(line) = line else { break };
                if let Ok(mut tail) = stderr.lock() {
                    tail.push_back(line);
                    if tail.len() > STDERR_LINES {
                        tail.pop_front();
                    }
                }
            }
        });
    }
    let mut session = Session {
        setup: &setup,
        rx: &rx,
        events: &events,
        transcript: &transcript,
        stdin,
        stderr,
        next_id: 1,
        session_id: String::new(),
        queue: VecDeque::new(),
        prompting: None,
        permissions: HashMap::new(),
        next_permission: 1,
        cost_usd: None,
    };
    let end = session.start().and_then(|()| {
        emit(AcpEvent::Ready {
            session: id,
            owner: owner.clone(),
        });
        session.serve()
    });
    drop(session);
    stop(&mut child);
    match end {
        Ok(End::Closed) | Err(End::Closed) => {
            note("Closed".into());
            AcpEvent::Closed { session: id, owner }
        }
        Ok(End::Failed(kind, message)) | Err(End::Failed(kind, message)) => {
            note(message.clone());
            AcpEvent::Failed {
                session: id,
                owner,
                kind,
                message,
            }
        }
    }
}

/// Ends the program: closing stdin ends a well-behaved agent; after a
/// moment the whole process tree goes (npx starts node under it).
fn stop(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    #[cfg(windows)]
    {
        _ = quiet(std::path::Path::new("taskkill"))
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .output();
    }
    _ = child.kill();
    _ = child.wait();
}

impl Session<'_> {
    fn emit(&self, event: AcpEvent) {
        _ = self.events.send_blocking(event);
    }

    fn record(&self, item: TranscriptItem) {
        if let Ok(mut t) = self.transcript.lock() {
            t.push(item);
        }
    }

    fn owner(&self) -> String {
        self.setup.owner.clone()
    }

    fn write(&mut self, message: &Value) -> Result<(), End> {
        let mut line = message.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|()| self.stdin.flush())
            .map_err(|_| self.crashed())
    }

    fn crashed(&self) -> End {
        let tail = self
            .stderr
            .lock()
            .map(|t| t.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();
        let mut message = format!("{} stopped", self.setup.program.name);
        if !tail.trim().is_empty() {
            message.push_str(&format!(": {}", tail.trim()));
        }
        End::Failed(FailureKind::Crashed, message)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<i64, End> {
        let id = self.next_id;
        self.next_id += 1;
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        Ok(id)
    }

    /// Sends a request and waits for its answer (setup only), handling what
    /// else arrives meanwhile.
    fn call(&mut self, method: &str, params: Value) -> Result<Value, End> {
        let id = self.request(method, params)?;
        let deadline = Instant::now() + SETUP_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let msg = match self.rx.recv_timeout(left) {
                Ok(msg) => msg,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(End::Failed(
                        FailureKind::Protocol,
                        format!("{} did not answer `{method}`", self.setup.program.name),
                    ));
                }
                Err(RecvTimeoutError::Disconnected) => return Err(End::Closed),
            };
            match msg {
                Msg::Incoming(value)
                    if value.get("id").and_then(Value::as_i64) == Some(id)
                        && value.get("method").is_none() =>
                {
                    if let Some(error) = value.get("error") {
                        return Err(classify(method, error));
                    }
                    return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                }
                Msg::Incoming(value) => self.incoming(value)?,
                Msg::Prompt(text) => self.queue.push_back(text),
                Msg::Close => return Err(End::Closed),
                Msg::Exited => return Err(self.crashed()),
                Msg::Cancel => self.queue.clear(),
                Msg::Answer { request, option } => self.answer(request, option)?,
            }
        }
    }

    /// `initialize`, `session/new`, and the permission mode.
    fn start(&mut self) -> Result<(), End> {
        self.call(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {
                    "fs": { "readTextFile": false, "writeTextFile": false },
                    "terminal": false
                },
                "clientInfo": { "name": "Carbon", "version": env!("CARGO_PKG_VERSION") }
            }),
        )?;
        let cwd = self.setup.cwd.display().to_string();
        let created = self.call("session/new", json!({ "cwd": cwd, "mcpServers": [] }))?;
        self.session_id = created
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| End::Failed(FailureKind::Protocol, "no session id".into()))?
            .to_string();
        let available: Vec<String> = created
            .pointer("/modes/availableModes")
            .and_then(Value::as_array)
            .map(|modes| {
                modes
                    .iter()
                    .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let current = created
            .pointer("/modes/currentModeId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let wanted = self
            .setup
            .mode
            .agent_modes()
            .iter()
            .find(|mode| available.iter().any(|a| a == *mode));
        if let Some(mode) = wanted
            && *mode != current
        {
            let session = self.session_id.clone();
            self.call(
                "session/set_mode",
                json!({ "sessionId": session, "modeId": mode }),
            )?;
        }
        self.record(TranscriptItem::Note(format!(
            "{} started ({})",
            self.setup.program.name,
            self.setup.mode.as_str()
        )));
        Ok(())
    }

    /// Runs prompts until closed.
    fn serve(&mut self) -> Result<End, End> {
        loop {
            if self.prompting.is_none()
                && let Some(text) = self.queue.pop_front()
            {
                self.send_prompt(text)?;
            }
            let Ok(msg) = self.rx.recv() else {
                return Ok(End::Closed);
            };
            match msg {
                Msg::Prompt(text) => self.queue.push_back(text),
                Msg::Incoming(value) => self.incoming(value)?,
                Msg::Answer { request, option } => self.answer(request, option)?,
                Msg::Cancel => {
                    self.queue.clear();
                    let waiting: Vec<u64> = self.permissions.keys().copied().collect();
                    for request in waiting {
                        self.answer(request, None)?;
                    }
                    if self.prompting.is_some() {
                        let session = self.session_id.clone();
                        self.write(&json!({
                            "jsonrpc": "2.0",
                            "method": "session/cancel",
                            "params": { "sessionId": session }
                        }))?;
                        self.record(TranscriptItem::Note("Cancelled".into()));
                    }
                }
                Msg::Close => return Ok(End::Closed),
                Msg::Exited => return Err(self.crashed()),
            }
        }
    }

    fn send_prompt(&mut self, text: String) -> Result<(), End> {
        self.record(TranscriptItem::Prompt(text.clone()));
        let session = self.session_id.clone();
        let id = self.request(
            "session/prompt",
            json!({ "sessionId": session, "prompt": [ { "type": "text", "text": text } ] }),
        )?;
        self.prompting = Some((id, String::new(), None));
        Ok(())
    }

    /// A line from the agent: a reply, a notification, or a request.
    fn incoming(&mut self, value: Value) -> Result<(), End> {
        let method = value.get("method").and_then(Value::as_str);
        let id = value.get("id").cloned();
        match (method, id) {
            // A reply: the running prompt's end.
            (None, Some(id)) => {
                let Some((prompt_id, text, _)) = self.prompting.take() else {
                    return Ok(());
                };
                if id.as_i64() != Some(prompt_id) {
                    self.prompting = Some((prompt_id, text, None));
                    return Ok(());
                }
                let (stop_reason, usage) = match value.get("error") {
                    Some(error) => {
                        let message = error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("error")
                            .to_string();
                        self.record(TranscriptItem::Note(format!("Prompt failed: {message}")));
                        ("error".to_string(), None)
                    }
                    None => {
                        let result = value.get("result").cloned().unwrap_or(Value::Null);
                        let stop = result
                            .get("stopReason")
                            .and_then(Value::as_str)
                            .unwrap_or("end_turn")
                            .to_string();
                        (stop, result.get("usage").map(|u| self.usage(u)))
                    }
                };
                let truncated = text.len() > MAX_TEXT;
                let mut text = text;
                if truncated {
                    let mut cut = MAX_TEXT;
                    while !text.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    text.truncate(cut);
                }
                self.emit(AcpEvent::PromptFinished {
                    session: self.setup.id,
                    owner: self.owner(),
                    stop_reason,
                    text,
                    truncated,
                    usage,
                });
            }
            (Some("session/update"), None) => {
                let update = value
                    .pointer("/params/update")
                    .cloned()
                    .unwrap_or(Value::Null);
                self.update(&update);
            }
            (Some("session/request_permission"), Some(id)) => {
                let params = value.get("params").cloned().unwrap_or(Value::Null);
                let call = params.get("toolCall").cloned().unwrap_or(Value::Null);
                let title = call
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("A tool")
                    .to_string();
                let kind = call
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("other")
                    .to_string();
                let options = params
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|options| {
                        options
                            .iter()
                            .map(|o| PermissionOption {
                                id: o["optionId"].as_str().unwrap_or_default().to_string(),
                                name: o["name"].as_str().unwrap_or_default().to_string(),
                                kind: o["kind"].as_str().unwrap_or_default().to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let request = self.next_permission;
                self.next_permission += 1;
                self.permissions.insert(request, (id, title.clone()));
                self.emit(AcpEvent::Permission {
                    session: self.setup.id,
                    owner: self.owner(),
                    request,
                    title,
                    kind,
                    options,
                });
            }
            // A request Carbon does not offer (fs, terminal: not advertised).
            (Some(method), Some(id)) => {
                self.write(&json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("{method} is not supported") }
                }))?;
            }
            // Other notifications (`_auth/status_update`, ...).
            _ => {}
        }
        Ok(())
    }

    fn update(&mut self, update: &Value) {
        let kind = update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match kind {
            "agent_message_chunk" => {
                let Some(text) = update.pointer("/content/text").and_then(Value::as_str) else {
                    return;
                };
                let message = update
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some((_, answer, last)) = self.prompting.as_mut() {
                    if !answer.is_empty() && message.is_some() && *last != message {
                        answer.push_str("\n\n");
                    }
                    if answer.len() <= MAX_TEXT {
                        answer.push_str(text);
                    }
                    *last = message;
                }
                self.record(TranscriptItem::Agent(text.to_string()));
                self.emit(AcpEvent::Text {
                    session: self.setup.id,
                    owner: self.owner(),
                    text: text.to_string(),
                });
            }
            "tool_call" | "tool_call_update" => {
                let id = update["toolCallId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let title = update["title"].as_str().unwrap_or_default().to_string();
                let status = update["status"].as_str().unwrap_or("pending").to_string();
                self.record(TranscriptItem::Tool {
                    id: id.clone(),
                    title: title.clone(),
                    status: status.clone(),
                });
                self.emit(AcpEvent::ToolCall {
                    session: self.setup.id,
                    owner: self.owner(),
                    id,
                    title,
                    status,
                });
            }
            "usage_update" => {
                if let Some(cost) = update.pointer("/cost/amount").and_then(Value::as_f64) {
                    self.cost_usd = Some(cost);
                }
                let usage = Usage {
                    input_tokens: update["used"].as_u64().unwrap_or_default(),
                    cost_usd: self.cost_usd,
                    ..Default::default()
                };
                self.emit(AcpEvent::Usage {
                    session: self.setup.id,
                    owner: self.owner(),
                    usage,
                });
            }
            _ => {}
        }
    }

    /// A prompt result's usage, with the session's cost so far.
    fn usage(&self, usage: &Value) -> Usage {
        let n = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or_default();
        Usage {
            input_tokens: n("inputTokens"),
            output_tokens: n("outputTokens"),
            cached_tokens: n("cachedReadTokens") + n("cachedWriteTokens"),
            cost_usd: self.cost_usd,
        }
    }

    /// Answers permission request `request` with an option, or cancels it.
    fn answer(&mut self, request: u64, option: Option<String>) -> Result<(), End> {
        let Some((id, title)) = self.permissions.remove(&request) else {
            return Ok(());
        };
        let outcome = match &option {
            Some(option) => json!({ "outcome": "selected", "optionId": option }),
            None => json!({ "outcome": "cancelled" }),
        };
        self.record(TranscriptItem::Permission {
            title,
            answer: option.unwrap_or_else(|| "cancelled".into()),
        });
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome } }))
    }
}

/// What a failed setup call means: sign-in needed, or a protocol problem.
fn classify(method: &str, error: &Value) -> End {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("error")
        .to_string();
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let lower = message.to_ascii_lowercase();
    if code == -32000
        || lower.contains("auth")
        || lower.contains("login")
        || lower.contains("sign in")
    {
        End::Failed(FailureKind::AuthRequired, message)
    } else {
        End::Failed(FailureKind::Protocol, format!("{method}: {message}"))
    }
}
