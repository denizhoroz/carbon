//! One language server process, spoken to over stdio with JSON-RPC
//! (`Content-Length` framed messages). A reader thread routes replies to the
//! requests waiting for them, answers the server's own requests, and passes
//! notifications on through a channel.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use serde_json::{Value, json};

use crate::{CoreError, Result};

/// A message the server sent on its own (not a reply).
#[derive(Clone, Debug)]
pub struct ServerNotification {
    pub method: String,
    pub params: Value,
}

type Pending = Arc<Mutex<HashMap<i64, async_channel::Sender<Result<Value>>>>>;

pub struct LanguageServer {
    name: String,
    child: Mutex<Child>,
    stdin: Arc<Mutex<ChildStdin>>,
    next_id: AtomicI64,
    pending: Pending,
    notifications: async_channel::Receiver<ServerNotification>,
    /// The last lines the server wrote to stderr, for error messages.
    stderr_tail: Arc<Mutex<Vec<String>>>,
}

impl LanguageServer {
    /// Starts `program` in `cwd`. The server is not initialized yet.
    pub fn spawn(
        name: &str,
        program: &Path,
        args: &[&str],
        env: &std::collections::BTreeMap<String, String>,
        cwd: &Path,
    ) -> Result<Arc<Self>> {
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(env)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            // No console window for the server.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|err| lsp_error(name, format!("cannot start {}: {err}", program.display())))?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().expect("piped stdin")));
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");

        let pending: Pending = Arc::default();
        let stderr_tail: Arc<Mutex<Vec<String>>> = Arc::default();
        let (sender, notifications) = async_channel::unbounded();
        {
            let pending = pending.clone();
            let stdin = stdin.clone();
            std::thread::Builder::new()
                .name(format!("lsp-{name}-read"))
                .spawn(move || read_loop(stdout, pending, stdin, sender))
                .map_err(|err| lsp_error(name, err.to_string()))?;
        }
        {
            let tail = stderr_tail.clone();
            std::thread::Builder::new()
                .name(format!("lsp-{name}-stderr"))
                .spawn(move || {
                    for line in BufReader::new(stderr).lines().map_while(|line| line.ok()) {
                        let mut tail = tail.lock().unwrap();
                        if tail.len() == 20 {
                            tail.remove(0);
                        }
                        tail.push(line);
                    }
                })
                .map_err(|err| lsp_error(name, err.to_string()))?;
        }
        Ok(Arc::new(Self {
            name: name.to_string(),
            child: Mutex::new(child),
            stdin,
            next_id: AtomicI64::new(1),
            pending,
            notifications,
            stderr_tail,
        }))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Messages the server sends on its own; closes when the server exits.
    pub fn notifications(&self) -> async_channel::Receiver<ServerNotification> {
        self.notifications.clone()
    }

    /// The server's last stderr lines (why it stopped, usually).
    pub fn stderr_tail(&self) -> String {
        self.stderr_tail.lock().unwrap().join("\n")
    }

    /// Sends request `R`; await the result on any executor.
    pub fn request<R: Request>(
        &self,
        params: R::Params,
    ) -> impl Future<Output = Result<R::Result>> + Send + 'static {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, reply) = async_channel::bounded(1);
        self.pending.lock().unwrap().insert(id, sender);
        let sent = serde_json::to_value(params)
            .map_err(|err| lsp_error(&self.name, err.to_string()))
            .and_then(|params| {
                self.write(
                    &json!({ "jsonrpc": "2.0", "id": id, "method": R::METHOD, "params": params }),
                )
            });
        if sent.is_err() {
            self.pending.lock().unwrap().remove(&id);
        }
        let name = self.name.clone();
        async move {
            sent?;
            let value = reply
                .recv()
                .await
                .map_err(|_| lsp_error(&name, "server stopped".into()))??;
            serde_json::from_value(value).map_err(|err| lsp_error(&name, err.to_string()))
        }
    }

    pub fn notify<N: Notification>(&self, params: N::Params) -> Result<()> {
        let params =
            serde_json::to_value(params).map_err(|err| lsp_error(&self.name, err.to_string()))?;
        self.write(&json!({ "jsonrpc": "2.0", "method": N::METHOD, "params": params }))
    }

    fn write(&self, message: &Value) -> Result<()> {
        write_message(&self.stdin, message).map_err(|err| lsp_error(&self.name, err.to_string()))
    }
}

/// Asks the server to exit, then makes sure it does.
impl Drop for LanguageServer {
    fn drop(&mut self) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        _ = self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": "shutdown" }));
        _ = self.write(&json!({ "jsonrpc": "2.0", "method": "exit" }));
        if let Ok(mut child) = self.child.lock() {
            _ = child.kill();
            _ = child.wait();
        }
    }
}

fn lsp_error(name: &str, message: String) -> CoreError {
    CoreError::Lsp(format!("{name}: {message}"))
}

fn write_message(stdin: &Mutex<ChildStdin>, message: &Value) -> std::io::Result<()> {
    let body = message.to_string();
    let mut stdin = stdin.lock().unwrap();
    write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    stdin.flush()
}

/// Reads one framed message; `None` at end of stream or on a broken frame.
pub(crate) fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn read_loop(
    stdout: impl Read,
    pending: Pending,
    stdin: Arc<Mutex<ChildStdin>>,
    notifications: async_channel::Sender<ServerNotification>,
) {
    let mut reader = BufReader::new(stdout);
    while let Some(message) = read_message(&mut reader) {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        match (method, id) {
            // A reply to one of our requests.
            (None, Some(id)) => {
                let Some(sender) = id
                    .as_i64()
                    .and_then(|id| pending.lock().unwrap().remove(&id))
                else {
                    continue;
                };
                let result = match message.get("error") {
                    Some(error) => Err(CoreError::Lsp(
                        error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("request failed")
                            .to_string(),
                    )),
                    None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                };
                _ = sender.try_send(result);
            }
            // A request from the server: answer it so it does not wait.
            (Some(method), Some(id)) => {
                let result = server_request_reply(method, message.get("params"));
                _ = write_message(
                    &stdin,
                    &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                );
            }
            (Some(method), None) => {
                let notification = ServerNotification {
                    method: method.to_string(),
                    params: message.get("params").cloned().unwrap_or(Value::Null),
                };
                if notifications.send_blocking(notification).is_err() {
                    break;
                }
            }
            (None, None) => {}
        }
    }
    // The server is gone: fail everything still waiting.
    pending.lock().unwrap().clear();
}

/// Replies to requests a server sends. Carbon has no settings to give yet,
/// so configuration items are all `null` (the server's defaults).
fn server_request_reply(method: &str, params: Option<&Value>) -> Value {
    match method {
        "workspace/configuration" => {
            let items = params
                .and_then(|params| params.get("items"))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            Value::Array(vec![Value::Null; items])
        }
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::read_message;

    #[test]
    fn reads_framed_messages_in_order() {
        let first = r#"{"jsonrpc":"2.0","id":1,"result":null}"#;
        let second = r#"{"jsonrpc":"2.0","method":"x","params":{"é":1}}"#;
        let stream = format!(
            "Content-Length: {}\r\n\r\n{first}Content-Length: {}\r\nContent-Type: x\r\n\r\n{second}",
            first.len(),
            second.len()
        );
        let mut reader = stream.as_bytes();
        assert_eq!(read_message(&mut reader).unwrap()["id"], 1);
        assert_eq!(read_message(&mut reader).unwrap()["params"]["é"], 1);
        assert!(read_message(&mut reader).is_none());
    }
}
