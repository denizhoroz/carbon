//! The host for extension code: WebAssembly components (`wasm` in the
//! manifest) run by wasmtime, one store per extension, against the WIT world
//! `carbon:extension` (`wit/since_v0.1.0`). An extension built for an older
//! API version keeps working while Carbon keeps that version's folder and
//! bindings here (Zed's pattern); this build has one.
//!
//! Sandbox: WASI with nothing shared (no folders, environment or sockets);
//! everything else goes through `host` functions that check the user's
//! grants. Every call has a time limit (epoch interruption) and the store a
//! memory limit; a call that traps or runs out returns an error and the GUI
//! turns the extension off. Calls block: make them off the UI thread.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use super::{Loaded, Widget, resolve_package_file};

mod v0_1_0 {
    wasmtime::component::bindgen!({
        path: "wit/since_v0.1.0",
        world: "extension",
    });
}

pub use v0_1_0::carbon::extension::host::ServerCommand;
use v0_1_0::carbon::extension::host::{self, ProcessOutput};

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

/// What an extension's code reports to the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostEvent {
    Log { extension: String, message: String },
    Notify { extension: String, message: String },
}

/// Things the app knows that extension code may ask for; shared with the
/// GUI, which keeps it current.
#[derive(Default)]
pub struct HostContext {
    pub workspace: Mutex<Option<PathBuf>>,
    /// Setting values as JSON text by full key (`<id>.<key>`).
    pub settings: Mutex<std::collections::HashMap<String, String>>,
    pub events: Mutex<Vec<HostEvent>>,
}

impl HostContext {
    /// Takes the events reported since the last call.
    pub fn take_events(&self) -> Vec<HostEvent> {
        std::mem::take(&mut self.events.lock().unwrap())
    }
}

/// The engine and the host functions, shared by every extension.
pub struct WasmRuntime {
    engine: Engine,
    linker: Linker<HostState>,
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
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|err| err.to_string())?;
        host::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |state| state)
            .map_err(|err| err.to_string())?;
        Ok(Self { engine, linker })
    }

    /// Compiles and starts `loaded`'s code with the capabilities in
    /// `grants`, then calls its `init`.
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
        let bindings = v0_1_0::Extension::instantiate(&mut store, &component, &self.linker)
            .map_err(|err| fault(format!("cannot start: {err:#}")))?;
        let extension = WasmExtension {
            store: Mutex::new(store),
            bindings,
        };
        extension.call(|bindings, store| bindings.call_init(store))?;
        Ok(extension)
    }
}

fn deadline_ticks() -> u64 {
    (CALL_LIMIT.as_millis() / TICK.as_millis()) as u64
}

/// A running extension.
pub struct WasmExtension {
    store: Mutex<Store<HostState>>,
    bindings: v0_1_0::Extension,
}

impl WasmExtension {
    /// Runs one export under the time limit; a trap (or the limit) is an
    /// error naming it.
    fn call<T>(
        &self,
        call: impl FnOnce(
            &v0_1_0::Extension,
            &mut Store<HostState>,
        ) -> wasmtime::Result<Result<T, String>>,
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

    pub fn run_command(&self, id: &str) -> Result<(), CallError> {
        self.call(|bindings, store| bindings.call_run_command(store, id))
    }

    pub fn language_server_command(
        &self,
        server: &str,
    ) -> Result<Option<ServerCommand>, CallError> {
        self.call(|bindings, store| bindings.call_language_server_command(store, server))
    }

    /// A dynamic view's widgets.
    pub fn render_view(&self, view: &str) -> Result<Vec<Widget>, CallError> {
        let json = self.call(|bindings, store| bindings.call_render_view(store, view))?;
        serde_json::from_str(&json)
            .map_err(|err| CallError::Returned(format!("view `{view}`: {err}")))
    }
}

/// One extension's side of the host.
pub struct HostState {
    extension: String,
    package: PathBuf,
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

impl host::Host for HostState {
    fn log(&mut self, message: String) {
        self.context.events.lock().unwrap().push(HostEvent::Log {
            extension: self.extension.clone(),
            message,
        });
    }

    fn notify(&mut self, message: String) {
        self.context.events.lock().unwrap().push(HostEvent::Notify {
            extension: self.extension.clone(),
            message,
        });
    }

    fn setting(&mut self, key: String) -> Option<String> {
        let full = format!("{}.{key}", self.extension);
        self.context.settings.lock().unwrap().get(&full).cloned()
    }

    fn workspace_root(&mut self) -> Option<String> {
        self.workspace().map(|root| root.display().to_string())
    }

    fn read_file(&mut self, path: String) -> Result<String, String> {
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

    fn run_process(&mut self, program: String, args: Vec<String>) -> Result<ProcessOutput, String> {
        let allowed =
            self.granted("process.exec:*") || self.granted(&format!("process.exec:{program}"));
        if !allowed {
            return Err(Self::denied(&format!("process.exec:{program}")));
        }
        let resolved =
            crate::lsp::find_program(&program).ok_or(format!("{program} is not on PATH"))?;
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
        Ok(ProcessOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn http_get(&mut self, url: String) -> Result<Vec<u8>, String> {
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
}
