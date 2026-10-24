//! The host for extension code: WebAssembly components (`wasm` in the
//! manifest) run by wasmtime, one store per extension, against the WIT world
//! `carbon:extension` of the API version the manifest asks for
//! (`wit/since_v0.1.0`, `wit/since_v0.2.0`). An extension built for an older
//! API keeps working while Carbon keeps that version's folder and bindings
//! here (Zed's pattern). Both versions share the host functions below.
//!
//! Sandbox: WASI with nothing shared (no folders, environment or sockets);
//! everything else goes through host functions that check the user's
//! grants. Every call has a time limit (epoch interruption) and the store a
//! memory limit; a call that traps or runs out returns an error and the GUI
//! turns the extension off. Calls block: make them off the UI thread.
//!
//! API 0.2 adds work that outlives a call: agent sessions (`acp`), jobs,
//! graph runs, approvals, timers. Host functions return at once; what they
//! start reports back through [`HostEvent`]s (for the GUI) and
//! [`ExtensionEvent`]s (for the extension's `on-event`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use semver::Version;
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use super::{Loaded, Widget, resolve_package_file, storage};
use crate::acp::{AcpManager, AgentProgram, PermissionMode, SessionOptions, Usage};
use crate::graph::{GraphDoc, GraphSpec, NodeState};

mod v0_1_0 {
    wasmtime::component::bindgen!({
        path: "wit/since_v0.1.0",
        world: "extension",
    });
}

mod v0_2_0 {
    wasmtime::component::bindgen!({
        path: "wit/since_v0.2.0",
        world: "extension",
    });
}

pub use v0_1_0::carbon::extension::host::ServerCommand;
use v0_2_0::carbon::extension as v2;

/// How often the epoch moves (the time limit's resolution).
const TICK: Duration = Duration::from_millis(10);
/// Longest a single call may run.
pub const CALL_LIMIT: Duration = Duration::from_secs(5);
/// Most memory one extension may use.
pub const MEMORY_LIMIT: usize = 256 * 1024 * 1024;
/// Largest response `http-get` returns.
const MAX_HTTP_BYTES: u64 = 32 * 1024 * 1024;

/// Why a call into extension code did not succeed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallError {
    /// The extension returned this error itself.
    Returned(String),
    /// Its code did not load, trapped, or ran out of time or memory: the
    /// app turns the extension off.
    Fault(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Returned(message) | Self::Fault(message) => f.write_str(message),
        }
    }
}

/// What an extension's code reports to the app (the GUI applies these).
#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    Log {
        extension: String,
        message: String,
    },
    Notify {
        extension: String,
        message: String,
    },
    JobStart {
        extension: String,
        job: u64,
        title: String,
    },
    JobProgress {
        job: u64,
        text: String,
    },
    JobFinish {
        job: u64,
        error: Option<String>,
    },
    RunBegin {
        extension: String,
        run: u64,
        path: PathBuf,
        job: Option<u64>,
    },
    NodeState {
        run: u64,
        node: String,
        state: NodeState,
        badge: Option<String>,
    },
    NodeOutput {
        run: u64,
        node: String,
        markdown: String,
    },
    RunLog {
        run: u64,
        node: Option<String>,
        line: String,
    },
    RunEnd {
        run: u64,
        error: Option<String>,
    },
    Approval {
        extension: String,
        id: u64,
        title: String,
        body: String,
        /// `(id, label, kind)`.
        options: Vec<(String, String, String)>,
        run: Option<u64>,
        node: Option<String>,
    },
    ApprovalWithdraw {
        id: u64,
    },
    Timer {
        extension: String,
        tag: String,
        after_ms: u32,
    },
}

/// What the app tells an extension's code (`on-event`, API 0.2).
#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionEvent {
    SessionReady(u64),
    SessionFailed {
        session: u64,
        kind: String,
        message: String,
    },
    AgentText {
        session: u64,
        text: String,
    },
    ToolCall {
        session: u64,
        title: String,
        status: String,
    },
    UsageUpdate {
        session: u64,
        usage: Usage,
    },
    PromptFinished {
        session: u64,
        stop_reason: String,
        text: String,
        truncated: bool,
        usage: Option<Usage>,
    },
    ApprovalAnswered {
        id: u64,
        option: String,
    },
    JobCancelRequested(u64),
    GraphAction {
        path: String,
        action: String,
        selection: Vec<String>,
    },
    TimerFired(String),
}

/// An agent program some extension declares, by its global id.
#[derive(Clone, Debug)]
pub struct AgentEntry {
    pub extension: String,
    pub id: String,
    pub program: AgentProgram,
}

/// Things the app knows that extension code may ask for; shared with the
/// GUI, which keeps it current.
#[derive(Default)]
pub struct HostContext {
    pub workspace: Mutex<Option<PathBuf>>,
    /// Setting values as JSON text by full key (`<id>.<key>`).
    pub settings: Mutex<HashMap<String, String>>,
    pub events: Mutex<Vec<HostEvent>>,
    /// Agent sessions (API 0.2 `acp`).
    pub acp: Mutex<Option<AcpManager>>,
    /// Every active extension's `[[agents]]`.
    pub agents: Mutex<Vec<AgentEntry>>,
    /// Where extension storage lives (`storage.rs`).
    pub storage: Mutex<Option<PathBuf>>,
    /// Ids for jobs, runs and approvals (one counter: never the same).
    next_id: AtomicU64,
    /// Agent sessions by job (Stop cancels them).
    pub session_jobs: Mutex<HashMap<u64, u64>>,
    /// Sessions whose text is streamed to their extension.
    pub streamed: Mutex<HashSet<u64>>,
    /// Sessions by the graph run and node they work for.
    pub session_nodes: Mutex<HashMap<u64, (u64, String)>>,
}

impl HostContext {
    /// Takes the events reported since the last call.
    pub fn take_events(&self) -> Vec<HostEvent> {
        std::mem::take(&mut self.events.lock().unwrap())
    }

    fn push(&self, event: HostEvent) {
        self.events.lock().unwrap().push(event);
    }

    fn new_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// The agent `id` names: `<extension>/<agent>`, or a bare agent id
    /// (`extension`'s own first).
    pub fn agent(&self, extension: &str, id: &str) -> Option<AgentEntry> {
        let agents = self.agents.lock().unwrap();
        if let Some((ext, agent)) = id.split_once('/') {
            return agents
                .iter()
                .find(|a| a.extension == ext && a.id == agent)
                .cloned();
        }
        agents
            .iter()
            .find(|a| a.extension == extension && a.id == id)
            .or_else(|| agents.iter().find(|a| a.id == id))
            .cloned()
    }
}

/// The engine and the host functions, shared by every extension.
pub struct WasmRuntime {
    engine: Engine,
    linker_v1: Linker<HostState>,
    linker_v2: Linker<HostState>,
}

/// One engine for the process, with a thread moving its epoch.
fn engine() -> Result<Engine, String> {
    static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let mut config = Config::new();
            config.wasm_component_model(true);
            config.epoch_interruption(true);
            let engine = Engine::new(&config).map_err(|err| err.to_string())?;
            let ticker = engine.weak();
            std::thread::Builder::new()
                .name("wasm-epoch".into())
                .spawn(move || {
                    while let Some(engine) = ticker.upgrade() {
                        engine.increment_epoch();
                        drop(engine);
                        std::thread::sleep(TICK);
                    }
                })
                .map_err(|err| err.to_string())?;
            Ok(engine)
        })
        .clone()
}

impl WasmRuntime {
    pub fn new() -> Result<Self, String> {
        let engine = engine()?;
        let mut linker_v1 = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker_v1).map_err(|err| err.to_string())?;
        v0_1_0::Extension::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker_v1, |s| s)
            .map_err(|err| err.to_string())?;
        let mut linker_v2 = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker_v2).map_err(|err| err.to_string())?;
        v0_2_0::Extension::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker_v2, |s| s)
            .map_err(|err| err.to_string())?;
        Ok(Self {
            engine,
            linker_v1,
            linker_v2,
        })
    }

    /// Compiles and starts `loaded`'s code with the capabilities in
    /// `grants`, against the API its manifest asks for, then calls `init`.
    pub fn load(
        &self,
        loaded: &Loaded,
        grants: BTreeSet<String>,
        context: Arc<HostContext>,
    ) -> Result<WasmExtension, CallError> {
        let fault = CallError::Fault;
        let path = loaded
            .def
            .wasm
            .as_deref()
            .ok_or_else(|| fault("the extension has no code".into()))?;
        let file = loaded.file(path).map_err(fault)?;
        let component = Component::from_file(&self.engine, &file)
            .map_err(|err| fault(format!("{}: {err:#}", file.display())))?;
        let state = HostState {
            extension: loaded.def.id.to_string(),
            package: loaded.dir.clone(),
            graphs: loaded.def.graphs.clone(),
            grants,
            context,
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY_LIMIT)
                .instances(64)
                .build(),
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.set_epoch_deadline(deadline_ticks());
        let v2 = loaded
            .def
            .api()
            .is_some_and(|api| api >= Version::new(0, 2, 0));
        let start = |err: wasmtime::Error| {
            fault(format!(
                "cannot start (built for another Carbon API than {}?): {err:#}",
                loaded.def.carbon_api
            ))
        };
        let bindings = if v2 {
            Bindings::V2(
                v0_2_0::Extension::instantiate(&mut store, &component, &self.linker_v2)
                    .map_err(start)?,
            )
        } else {
            Bindings::V1(
                v0_1_0::Extension::instantiate(&mut store, &component, &self.linker_v1)
                    .map_err(start)?,
            )
        };
        let extension = WasmExtension {
            store: Mutex::new(store),
            bindings,
        };
        extension.call(|bindings, store| match bindings {
            Bindings::V1(b) => b.call_init(store),
            Bindings::V2(b) => b.call_init(store),
        })?;
        Ok(extension)
    }
}

fn deadline_ticks() -> u64 {
    (CALL_LIMIT.as_millis() / TICK.as_millis()) as u64
}

enum Bindings {
    V1(v0_1_0::Extension),
    V2(v0_2_0::Extension),
}

/// A running extension.
pub struct WasmExtension {
    store: Mutex<Store<HostState>>,
    bindings: Bindings,
}

impl WasmExtension {
    /// Runs one export under the time limit; a trap (or the limit) is an
    /// error naming it.
    fn call<T>(
        &self,
        call: impl FnOnce(&Bindings, &mut Store<HostState>) -> wasmtime::Result<Result<T, String>>,
    ) -> Result<T, CallError> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| CallError::Fault("the extension failed before".into()))?;
        store.set_epoch_deadline(deadline_ticks());
        match call(&self.bindings, &mut store) {
            Ok(result) => result.map_err(CallError::Returned),
            Err(trap) => {
                let interrupted = trap
                    .downcast_ref::<wasmtime::Trap>()
                    .is_some_and(|trap| *trap == wasmtime::Trap::Interrupt);
                Err(CallError::Fault(if interrupted {
                    format!("took longer than {} s", CALL_LIMIT.as_secs())
                } else {
                    format!("failed: {trap:#}")
                }))
            }
        }
    }

    /// Whether it gets events (API 0.2).
    pub fn has_events(&self) -> bool {
        matches!(self.bindings, Bindings::V2(_))
    }

    pub fn run_command(&self, id: &str) -> Result<(), CallError> {
        self.call(|bindings, store| match bindings {
            Bindings::V1(b) => b.call_run_command(store, id),
            Bindings::V2(b) => b.call_run_command(store, id),
        })
    }

    pub fn language_server_command(
        &self,
        server: &str,
    ) -> Result<Option<ServerCommand>, CallError> {
        self.call(|bindings, store| match bindings {
            Bindings::V1(b) => b.call_language_server_command(store, server),
            Bindings::V2(b) => b.call_language_server_command(store, server).map(|r| {
                r.map(|command| {
                    command.map(|c| ServerCommand {
                        program: c.program,
                        args: c.args,
                        env: c.env,
                    })
                })
            }),
        })
    }

    /// A dynamic view's widgets.
    pub fn render_view(&self, view: &str) -> Result<Vec<Widget>, CallError> {
        let json = self.call(|bindings, store| match bindings {
            Bindings::V1(b) => b.call_render_view(store, view),
            Bindings::V2(b) => b.call_render_view(store, view),
        })?;
        serde_json::from_str(&json)
            .map_err(|err| CallError::Returned(format!("view `{view}`: {err}")))
    }

    /// Tells the code what happened (API 0.2; others ignore events).
    pub fn on_event(&self, event: &ExtensionEvent) -> Result<(), CallError> {
        let event = wit_event(event);
        self.call(|bindings, store| match bindings {
            Bindings::V1(_) => Ok(Ok(())),
            Bindings::V2(b) => b.call_on_event(store, &event),
        })
    }

    /// The code's own problems with a graph (API 0.2), as JSON text.
    pub fn check_graph(&self, path: &str, doc: &GraphDoc) -> Result<String, CallError> {
        let doc = wit_graph(path, doc);
        self.call(|bindings, store| match bindings {
            Bindings::V1(_) => Ok(Ok("[]".into())),
            Bindings::V2(b) => b.call_check_graph(store, &doc),
        })
    }
}

fn wit_usage(usage: &Usage) -> v2::events::Usage {
    v2::events::Usage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_tokens: usage.cached_tokens,
        cost_usd: usage.cost_usd,
    }
}

fn wit_event(event: &ExtensionEvent) -> v2::events::Event {
    use v2::events::{self as e, Event};
    match event {
        ExtensionEvent::SessionReady(id) => Event::SessionReady(*id),
        ExtensionEvent::SessionFailed {
            session,
            kind,
            message,
        } => Event::SessionFailed(e::SessionFailed {
            session: *session,
            kind: kind.clone(),
            message: message.clone(),
        }),
        ExtensionEvent::AgentText { session, text } => Event::AgentText((*session, text.clone())),
        ExtensionEvent::ToolCall {
            session,
            title,
            status,
        } => Event::ToolCall((*session, title.clone(), status.clone())),
        ExtensionEvent::UsageUpdate { session, usage } => {
            Event::UsageUpdate((*session, wit_usage(usage)))
        }
        ExtensionEvent::PromptFinished {
            session,
            stop_reason,
            text,
            truncated,
            usage,
        } => Event::PromptFinished(e::PromptFinished {
            session: *session,
            stop_reason: stop_reason.clone(),
            text: text.clone(),
            truncated: *truncated,
            usage: usage.as_ref().map(wit_usage),
        }),
        ExtensionEvent::ApprovalAnswered { id, option } => {
            Event::ApprovalAnswered(e::ApprovalAnswered {
                id: *id,
                choice: option.clone(),
            })
        }
        ExtensionEvent::JobCancelRequested(job) => Event::JobCancelRequested(*job),
        ExtensionEvent::GraphAction {
            path,
            action,
            selection,
        } => Event::GraphAction(e::GraphAction {
            path: path.clone(),
            action: action.clone(),
            selection: selection.clone(),
        }),
        ExtensionEvent::TimerFired(tag) => Event::TimerFired(tag.clone()),
    }
}

fn wit_graph(path: &str, doc: &GraphDoc) -> v2::graphs::GraphDoc {
    let meta: serde_json::Map<String, serde_json::Value> = doc
        .meta
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    v2::graphs::GraphDoc {
        path: path.to_string(),
        kind: doc.kind.clone(),
        name: doc.name.clone(),
        meta: serde_json::Value::Object(meta).to_string(),
        nodes: doc
            .nodes
            .iter()
            .map(|(id, node)| v2::graphs::Node {
                id: id.clone(),
                kind: node.kind.clone(),
                title: node.title.clone(),
                disabled: node.disabled,
                config: serde_json::Value::Object(
                    node.config
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                )
                .to_string(),
            })
            .collect(),
        edges: doc
            .edges
            .iter()
            .map(|edge| v2::graphs::Edge {
                id: edge.id.clone(),
                from_node: edge.from.node.clone(),
                from_port: edge.from.port.clone(),
                to_node: edge.to.node.clone(),
                to_port: edge.to.port.clone(),
            })
            .collect(),
    }
}

/// One extension's side of the host.
pub struct HostState {
    extension: String,
    package: PathBuf,
    /// Its own graph kinds (`[[graphs]]`).
    graphs: Vec<GraphSpec>,
    grants: BTreeSet<String>,
    context: Arc<HostContext>,
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// `path` inside `root`, refusing `..` and links out of it.
fn inside(root: &Path, path: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(path);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let real = dunce::canonicalize(&joined).map_err(|err| format!("{path}: {err}"))?;
    let root = dunce::canonicalize(root).map_err(|err| format!("{}: {err}", root.display()))?;
    if real.starts_with(&root) {
        Ok(real)
    } else {
        Err(format!("{path} is outside {}", root.display()))
    }
}

/// Like [`inside`], for a file that may not exist yet: its parts must be
/// plain names (no `..`), and the deepest existing folder must be inside.
fn inside_new(root: &Path, path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "{path}: use a path inside the open folder, without `..`"
        ));
    }
    let root = dunce::canonicalize(root).map_err(|err| format!("{}: {err}", root.display()))?;
    let target = root.join(relative);
    let mut existing = target.parent().map(Path::to_path_buf);
    while let Some(dir) = existing.clone() {
        if dir.exists() {
            let real = dunce::canonicalize(&dir).map_err(|err| format!("{path}: {err}"))?;
            if !real.starts_with(&root) {
                return Err(format!("{path} is outside {}", root.display()));
            }
            break;
        }
        existing = dir.parent().map(Path::to_path_buf);
    }
    Ok(target)
}

/// The shared host functions (both API versions call these).
impl HostState {
    fn granted(&self, capability: &str) -> bool {
        self.grants.contains(capability)
    }

    fn denied(capability: &str) -> String {
        format!("not allowed: needs the `{capability}` capability")
    }

    fn workspace(&self) -> Option<PathBuf> {
        self.context.workspace.lock().unwrap().clone()
    }

    fn do_log(&mut self, message: String) {
        self.context.push(HostEvent::Log {
            extension: self.extension.clone(),
            message,
        });
    }

    fn do_notify(&mut self, message: String) {
        self.context.push(HostEvent::Notify {
            extension: self.extension.clone(),
            message,
        });
    }

    fn do_setting(&mut self, key: String) -> Option<String> {
        let full = format!("{}.{key}", self.extension);
        self.context.settings.lock().unwrap().get(&full).cloned()
    }

    fn do_workspace_root(&mut self) -> Option<String> {
        self.workspace().map(|root| root.display().to_string())
    }

    fn do_read_file(&mut self, path: String) -> Result<String, String> {
        let file = if self.granted("fs.read:workspace")
            && let Some(root) = self.workspace()
            && let Ok(file) = inside(&root, &path)
        {
            file
        } else if self.granted("fs.read:package") {
            resolve_package_file(&self.package, &path)?
        } else {
            return Err(Self::denied("fs.read:workspace"));
        };
        std::fs::read_to_string(&file).map_err(|err| format!("{path}: {err}"))
    }

    fn do_write_file(&mut self, path: String, contents: String) -> Result<(), String> {
        if !self.granted("fs.write:workspace") {
            return Err(Self::denied("fs.write:workspace"));
        }
        let root = self.workspace().ok_or("no folder is open")?;
        let file = inside_new(&root, &path)?;
        let dir = file.parent().unwrap_or(&root).to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|err| format!("{path}: {err}"))?;
        let mut temp =
            tempfile::NamedTempFile::new_in(&dir).map_err(|err| format!("{path}: {err}"))?;
        use std::io::Write as _;
        temp.write_all(contents.as_bytes())
            .map_err(|err| format!("{path}: {err}"))?;
        temp.persist(&file)
            .map_err(|err| format!("{path}: {}", err.error))?;
        Ok(())
    }

    /// `(status, stdout, stderr)`.
    fn do_run_process(
        &mut self,
        program: String,
        args: Vec<String>,
    ) -> Result<(Option<i32>, String, String), String> {
        let allowed =
            self.granted("process.exec:*") || self.granted(&format!("process.exec:{program}"));
        if !allowed {
            return Err(Self::denied(&format!("process.exec:{program}")));
        }
        let resolved = crate::acp::find_on_path(&program)
            .or_else(|| crate::lsp::find_program(&program))
            .ok_or(format!("{program} is not on PATH"))?;
        let mut command = std::process::Command::new(resolved);
        command.args(&args);
        if let Some(root) = self.workspace() {
            command.current_dir(root);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let output = command
            .output()
            .map_err(|err| format!("{program}: {err}"))?;
        Ok((
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }

    fn do_http_get(&mut self, url: String) -> Result<Vec<u8>, String> {
        let host = url
            .strip_prefix("https://")
            .and_then(|rest| rest.split(['/', '?', '#', ':']).next())
            .filter(|host| !host.is_empty())
            .map(str::to_ascii_lowercase)
            .ok_or("only https:// URLs")?;
        let allowed = self.grants.iter().any(|grant| {
            grant.strip_prefix("net:").is_some_and(|scope| {
                scope == "*"
                    || scope == host
                    || scope
                        .strip_prefix("*.")
                        .is_some_and(|domain| host.ends_with(&format!(".{domain}")))
            })
        });
        if !allowed {
            return Err(Self::denied(&format!("net:{host}")));
        }
        let response = reqwest::blocking::Client::builder()
            .user_agent(concat!("Carbon/", env!("CARGO_PKG_VERSION")))
            .timeout(CALL_LIMIT)
            .build()
            .and_then(|client| client.get(&url).send())
            .and_then(|response| response.error_for_status())
            .map_err(|err| format!("{url}: {err}"))?;
        let mut body = Vec::new();
        use std::io::Read as _;
        response
            .take(MAX_HTTP_BYTES)
            .read_to_end(&mut body)
            .map_err(|err| format!("{url}: {err}"))?;
        Ok(body)
    }

    fn storage_file(&self) -> Option<PathBuf> {
        let root = self.context.storage.lock().unwrap().clone()?;
        Some(storage::storage_file(
            &root,
            &self.extension,
            self.workspace().as_deref(),
        ))
    }

    /// An agent session of this extension (others are refused).
    fn own_session(&self, session: u64) -> Result<AcpManager, String> {
        let manager = self
            .context
            .acp
            .lock()
            .unwrap()
            .clone()
            .ok_or("agents are not available")?;
        if manager.owner_of(session).as_deref() != Some(self.extension.as_str()) {
            return Err(format!("no session {session}"));
        }
        Ok(manager)
    }

    /// A graph file of one of this extension's graph kinds, read.
    fn own_graph(&self, path: &str) -> Result<(PathBuf, GraphDoc), String> {
        let root = self.workspace().ok_or("no folder is open")?;
        let file = inside(&root, path)?;
        let doc = crate::graph::read_file(&file)?;
        let mine = self
            .graphs
            .iter()
            .any(|g| doc.kind == format!("{}/{}", self.extension, g.id));
        if !mine {
            return Err(format!("{path} is not one of {}'s graphs", self.extension));
        }
        Ok((file, doc))
    }
}

// ---- API 0.1 ------------------------------------------------------------

impl v0_1_0::carbon::extension::host::Host for HostState {
    fn log(&mut self, message: String) {
        self.do_log(message)
    }

    fn notify(&mut self, message: String) {
        self.do_notify(message)
    }

    fn setting(&mut self, key: String) -> Option<String> {
        self.do_setting(key)
    }

    fn workspace_root(&mut self) -> Option<String> {
        self.do_workspace_root()
    }

    fn read_file(&mut self, path: String) -> Result<String, String> {
        self.do_read_file(path)
    }

    fn run_process(
        &mut self,
        program: String,
        args: Vec<String>,
    ) -> Result<v0_1_0::carbon::extension::host::ProcessOutput, String> {
        let (status, stdout, stderr) = self.do_run_process(program, args)?;
        Ok(v0_1_0::carbon::extension::host::ProcessOutput {
            status,
            stdout,
            stderr,
        })
    }

    fn http_get(&mut self, url: String) -> Result<Vec<u8>, String> {
        self.do_http_get(url)
    }
}

// ---- API 0.2 ------------------------------------------------------------

impl v2::host::Host for HostState {
    fn log(&mut self, message: String) {
        self.do_log(message)
    }

    fn notify(&mut self, message: String) {
        self.do_notify(message)
    }

    fn setting(&mut self, key: String) -> Option<String> {
        self.do_setting(key)
    }

    fn workspace_root(&mut self) -> Option<String> {
        self.do_workspace_root()
    }

    fn read_file(&mut self, path: String) -> Result<String, String> {
        self.do_read_file(path)
    }

    fn write_file(&mut self, path: String, contents: String) -> Result<(), String> {
        self.do_write_file(path, contents)
    }

    fn run_process(
        &mut self,
        program: String,
        args: Vec<String>,
    ) -> Result<v2::host::ProcessOutput, String> {
        let (status, stdout, stderr) = self.do_run_process(program, args)?;
        Ok(v2::host::ProcessOutput {
            status,
            stdout,
            stderr,
        })
    }

    fn http_get(&mut self, url: String) -> Result<Vec<u8>, String> {
        self.do_http_get(url)
    }

    fn storage_get(&mut self, key: String) -> Option<String> {
        storage::get(&self.storage_file()?, &key)
    }

    fn storage_set(&mut self, key: String, value: String) -> Result<(), String> {
        let file = self.storage_file().ok_or("storage is not available")?;
        storage::set(&file, &key, &value)
    }

    fn storage_delete(&mut self, key: String) {
        if let Some(file) = self.storage_file() {
            storage::delete(&file, &key);
        }
    }

    fn storage_keys(&mut self, prefix: String) -> Vec<String> {
        self.storage_file()
            .map(|file| storage::keys(&file, &prefix))
            .unwrap_or_default()
    }

    fn now_ms(&mut self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default()
    }

    fn set_timer(&mut self, after_ms: u32, tag: String) {
        self.context.push(HostEvent::Timer {
            extension: self.extension.clone(),
            tag,
            after_ms,
        });
    }
}

impl v2::jobs::Host for HostState {
    fn start_job(&mut self, title: String) -> u64 {
        let job = self.context.new_id();
        self.context.push(HostEvent::JobStart {
            extension: self.extension.clone(),
            job,
            title,
        });
        job
    }

    fn set_job_progress(&mut self, job: u64, text: String) {
        self.context.push(HostEvent::JobProgress { job, text });
    }

    fn finish_job(&mut self, job: u64, error: Option<String>) {
        self.context.push(HostEvent::JobFinish { job, error });
    }
}

impl v2::acp::Host for HostState {
    fn list_agents(&mut self) -> Vec<v2::acp::AgentInfo> {
        self.context
            .agents
            .lock()
            .unwrap()
            .iter()
            .map(|entry| v2::acp::AgentInfo {
                id: format!("{}/{}", entry.extension, entry.id),
                name: entry.program.name.clone(),
                available: crate::acp::find_on_path(&entry.program.command).is_some()
                    || entry.program.npm.is_some(),
                hint: entry.program.install_hint.clone(),
            })
            .collect()
    }

    fn start_session(&mut self, options: v2::acp::SessionOptions) -> Result<u64, String> {
        if !self.granted("agent.invoke") {
            return Err(Self::denied("agent.invoke"));
        }
        let manager = self
            .context
            .acp
            .lock()
            .unwrap()
            .clone()
            .ok_or("agents are not available")?;
        let entry = self
            .context
            .agent(&self.extension, &options.agent)
            .ok_or_else(|| format!("no agent `{}`", options.agent))?;
        let root = self.workspace().ok_or("no folder is open")?;
        let cwd = match &options.cwd {
            Some(dir) => inside(&root, dir)?,
            None => root,
        };
        let mode = PermissionMode::parse(&options.permission_mode)
            .ok_or_else(|| format!("unknown permission mode `{}`", options.permission_mode))?;
        let session = manager.start_session(
            entry.program,
            SessionOptions {
                owner: self.extension.clone(),
                cwd,
                mode,
            },
        );
        if let Some(job) = options.job {
            self.context
                .session_jobs
                .lock()
                .unwrap()
                .insert(session, job);
        }
        if options.stream_text {
            self.context.streamed.lock().unwrap().insert(session);
        }
        if let (Some(run), Some(node)) = (options.run, options.node) {
            self.context
                .session_nodes
                .lock()
                .unwrap()
                .insert(session, (run, node));
        }
        Ok(session)
    }

    fn prompt(&mut self, session: u64, text: String) -> Result<(), String> {
        self.own_session(session)?.prompt(session, text)
    }

    fn cancel(&mut self, session: u64) {
        if let Ok(manager) = self.own_session(session) {
            manager.cancel(session);
        }
    }

    fn close_session(&mut self, session: u64) {
        if let Ok(manager) = self.own_session(session) {
            manager.close(session);
        }
    }
}

impl v2::graphs::Host for HostState {
    fn list_graphs(&mut self, graph: String) -> Vec<String> {
        let (Some(spec), Some(root)) = (
            self.graphs.iter().find(|g| g.id == graph).cloned(),
            self.workspace(),
        ) else {
            return Vec::new();
        };
        let ending = format!(".{}", spec.suffix.to_ascii_lowercase());
        let dir = root.join(&spec.dir);
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.to_ascii_lowercase()
                    .ends_with(&ending)
                    .then(|| format!("{}/{name}", spec.dir))
            })
            .collect();
        files.sort();
        files
    }

    fn read_graph(&mut self, path: String) -> Result<v2::graphs::GraphDoc, String> {
        let (_, doc) = self.own_graph(&path)?;
        Ok(wit_graph(&path, &doc))
    }

    fn begin_run(&mut self, path: String, job: Option<u64>) -> u64 {
        let run = self.context.new_id();
        let file = self
            .workspace()
            .and_then(|root| inside(&root, &path).ok())
            .unwrap_or_else(|| PathBuf::from(&path));
        self.context.push(HostEvent::RunBegin {
            extension: self.extension.clone(),
            run,
            path: file,
            job,
        });
        run
    }

    fn set_node_state(&mut self, run: u64, node: String, state: String, badge: Option<String>) {
        let state = match state.as_str() {
            "queued" => NodeState::Queued,
            "running" => NodeState::Running,
            "waiting" => NodeState::Waiting,
            "done" => NodeState::Done,
            "failed" => NodeState::Failed,
            "skipped" => NodeState::Skipped,
            "cancelled" => NodeState::Cancelled,
            _ => NodeState::Idle,
        };
        self.context.push(HostEvent::NodeState {
            run,
            node,
            state,
            badge,
        });
    }

    fn set_node_output(&mut self, run: u64, node: String, markdown: String) {
        self.context.push(HostEvent::NodeOutput {
            run,
            node,
            markdown,
        });
    }

    fn run_log(&mut self, run: u64, node: Option<String>, line: String) {
        self.context.push(HostEvent::RunLog { run, node, line });
    }

    fn end_run(&mut self, run: u64, error: Option<String>) {
        self.context.push(HostEvent::RunEnd { run, error });
    }
}

impl v2::approvals::Host for HostState {
    fn request(&mut self, request: v2::approvals::ApprovalRequest) -> u64 {
        let id = self.context.new_id();
        self.context.push(HostEvent::Approval {
            extension: self.extension.clone(),
            id,
            title: request.title,
            body: request.body,
            options: request
                .options
                .into_iter()
                .map(|o| (o.id, o.label, o.kind))
                .collect(),
            run: request.run,
            node: request.node,
        });
        id
    }

    fn withdraw(&mut self, id: u64) {
        self.context.push(HostEvent::ApprovalWithdraw { id });
    }
}

impl v2::events::Host for HostState {}
