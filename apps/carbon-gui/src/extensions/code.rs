//! Extension code in the GUI: each extension's WebAssembly component is
//! loaded the first time something needs it (a command, a dynamic view),
//! off the UI thread, with the capabilities the user granted (all it asks
//! for when linked for development). A call that fails by trapping or
//! running out of time turns the extension off (quarantine) until the user
//! turns it on again. Log lines go to stderr; `notify` shows a notification.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use carbon_core::extension::{
    CallError, ExtensionId, HostContext, HostEvent, Origin, WasmExtension, WasmRuntime, Widget,
};
use gpui_kit::component::WindowExt as _;
use gpui_kit::*;

/// The runtime, made on first use (or why it could not be).
type RuntimeSlot = Arc<Mutex<Option<Result<Arc<WasmRuntime>, CallError>>>>;

/// Loaded code and what the host shares with it.
pub struct ExtensionCode {
    runtime: RuntimeSlot,
    loaded: Arc<Mutex<HashMap<ExtensionId, Arc<WasmExtension>>>>,
    context: Arc<HostContext>,
    /// Moves after every call that ran code (dynamic views redraw).
    pub runs: u64,
}

impl Global for ExtensionCode {}

pub fn init(cx: &mut App) {
    cx.set_global(ExtensionCode {
        runtime: Arc::default(),
        loaded: Arc::default(),
        context: Arc::default(),
        runs: 0,
    });
    cx.observe_global::<crate::settings::AppSettings>(sync_settings)
        .detach();
    sync_settings(cx);
}

/// Copies extension settings into the host context (code reads them there).
fn sync_settings(cx: &mut App) {
    let store = crate::settings::get(cx);
    let values: HashMap<String, String> = store
        .definitions()
        .iter()
        .filter(|setting| setting.extension.is_some())
        .map(|setting| (setting.key.clone(), store.get(&setting.key).to_string()))
        .collect();
    *cx.global::<ExtensionCode>()
        .context
        .settings
        .lock()
        .unwrap() = values;
}

/// The open folder, for code that asks (`workspace-root`, `read-file`).
pub fn set_workspace(root: Option<std::path::PathBuf>, cx: &App) {
    *cx.global::<ExtensionCode>()
        .context
        .workspace
        .lock()
        .unwrap() = root;
}

/// Forgets loaded code (extensions changed: removed ones stop, changed
/// ones load again on next use).
pub fn unload_all(cx: &App) {
    cx.global::<ExtensionCode>().loaded.lock().unwrap().clear();
}

/// What a call on an extension does.
type Call<T> = Box<dyn FnOnce(&WasmExtension) -> Result<T, CallError> + Send>;

/// Runs `call` on extension `id`'s code off the UI thread, loading it first
/// if needed. A failure of the code itself turns the extension off.
fn with_code<T: Send + 'static>(
    id: ExtensionId,
    call: Call<T>,
    cx: &mut App,
) -> Task<Result<T, String>> {
    let registry = super::registry(cx);
    let Some(loaded) = registry.get(&id).cloned() else {
        return Task::ready(Err(format!("{id} is not installed")));
    };
    if !registry.is_active(&id) {
        return Task::ready(Err(format!("{} is turned off", loaded.def.name)));
    }
    let grants: BTreeSet<String> = match loaded.origin {
        Origin::Dev => loaded.def.capabilities.iter().cloned().collect(),
        Origin::Installed => super::state(cx)
            .grants
            .get(&id)
            .cloned()
            .unwrap_or_default(),
    };
    let code = cx.global::<ExtensionCode>();
    let (runtime, cache, context) = (
        code.runtime.clone(),
        code.loaded.clone(),
        code.context.clone(),
    );
    let work = cx.background_spawn({
        let id = id.clone();
        async move {
            let existing = cache.lock().unwrap().get(&id).cloned();
            let extension = match existing {
                Some(extension) => extension,
                None => {
                    let runtime = runtime
                        .lock()
                        .unwrap()
                        .get_or_insert_with(|| {
                            WasmRuntime::new().map(Arc::new).map_err(CallError::Fault)
                        })
                        .clone()?;
                    let extension = Arc::new(runtime.load(&loaded, grants, context)?);
                    cache.lock().unwrap().insert(id.clone(), extension.clone());
                    extension
                }
            };
            call(&extension)
        }
    });
    cx.spawn(async move |cx| {
        let result = work.await;
        cx.update(|cx| {
            cx.update_global::<ExtensionCode, _>(|code, _| code.runs += 1);
            report_events(cx);
            if let Err(CallError::Fault(why)) = &result {
                cx.global::<ExtensionCode>()
                    .loaded
                    .lock()
                    .unwrap()
                    .remove(&id);
                let why = why.clone();
                _ = super::update_state(cx, |state| {
                    state.quarantined.insert(id.clone(), why);
                });
            }
        });
        result.map_err(|err| err.to_string())
    })
}

/// Shows what code reported: notifications in the active window, log lines
/// on stderr.
fn report_events(cx: &mut App) {
    let events = cx.global::<ExtensionCode>().context.take_events();
    for event in events {
        match event {
            HostEvent::Log { extension, message } => eprintln!("[{extension}] {message}"),
            HostEvent::Notify { message, .. } => {
                // The active window, else the first (Carbon has one for now).
                let window = cx.active_window().or_else(|| cx.windows().first().copied());
                if let Some(window) = window {
                    _ = window.update(cx, |_, window, cx| window.push_notification(message, cx));
                }
            }
        }
    }
}

/// Runs a command of extension `id` that its code handles.
pub fn run_command(id: ExtensionId, command: String, cx: &mut App) -> Task<Result<(), String>> {
    with_code(id, Box::new(move |code| code.run_command(&command)), cx)
}

/// The command extension `id`'s code gives for one of its language servers.
pub fn language_server_command(
    id: ExtensionId,
    server: String,
    cx: &mut App,
) -> Task<Result<Option<carbon_core::extension::ServerCommand>, String>> {
    with_code(
        id,
        Box::new(move |code| code.language_server_command(&server)),
        cx,
    )
}

/// A dynamic view's widgets.
pub fn render_view(
    id: ExtensionId,
    view: String,
    cx: &mut App,
) -> Task<Result<Vec<Widget>, String>> {
    with_code(id, Box::new(move |code| code.render_view(&view)), cx)
}

/// Moves after every call that ran extension code.
pub fn runs(cx: &App) -> u64 {
    cx.global::<ExtensionCode>().runs
}
