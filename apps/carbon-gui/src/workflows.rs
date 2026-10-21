//! The workflow registry as the GUI sees it (`project/built/workflows.md`):
//! Carbon's own, installed and user workflows plus overlays, with disabled
//! and quarantined ones blocked. Loaded at start; `reload` rebuilds it after
//! Manage Workflows changes something.
//! Also: the crash-loop guard at start, safe mode, the panel kinds manifests
//! may name, workflow icons and workflow commands.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use carbon_core::workflow::{
    CommandDef, Registry, StateFile, WorkflowDef, WorkflowDirs, WorkflowId, take_unfinished,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{Action, ActionBuildError, App, AssetSource, SharedString};

use crate::app_assets::AppAssets;

/// Panel kinds a manifest may name; `layouts::PanelKind` builds them.
#[cfg(not(feature = "webview"))]
pub const PANEL_KINDS: &[&str] = &[
    "editor",
    "explorer",
    "terminal",
    "chat",
    "placeholder",
    "view",
];
/// Panel kinds a manifest may name; `layouts::PanelKind` builds them.
#[cfg(feature = "webview")]
pub const PANEL_KINDS: &[&str] = &[
    "editor",
    "explorer",
    "terminal",
    "chat",
    "placeholder",
    "view",
    "webview",
];

pub struct Workflows {
    registry: RwLock<Arc<Registry>>,
    /// `None` in safe mode: nothing is read from or written to the folder.
    dirs: Option<WorkflowDirs>,
    /// Shown as notifications once the window is open.
    notices: Vec<String>,
    /// `state.json` as read at start; the workbench takes it once.
    state: Mutex<Option<StateFile>>,
}

static WORKFLOWS: OnceLock<Workflows> = OnceLock::new();

/// Loads the workflows. `safe_mode`: `--safe-mode` was given. Call once,
/// before the window opens; later calls change nothing.
pub fn init(safe_mode: bool) {
    let _ = WORKFLOWS.set(load(safe_mode));
}

fn get() -> &'static Workflows {
    // Without `init` (unit tests): built-ins only, no folder.
    WORKFLOWS.get_or_init(|| Workflows {
        registry: RwLock::new(Arc::new(Registry::builtin(PANEL_KINDS))),
        dirs: None,
        notices: Vec::new(),
        state: Mutex::new(None),
    })
}

fn load(mut safe_mode: bool) -> Workflows {
    let mut notices = Vec::new();
    let found = WorkflowDirs::from_env();
    // A workflow that was being opened when Carbon last stopped.
    let unfinished = found.as_ref().and_then(take_unfinished);
    if let Some(id) = &unfinished
        && id.is_builtin()
        && !safe_mode
    {
        safe_mode = true;
        let builtins = Registry::builtin(PANEL_KINDS);
        let name = builtins
            .get(id)
            .map_or(id.as_str(), |def| def.name.as_str());
        notices.push(format!(
            "Carbon stopped while opening \"{name}\" last time, so it started in safe mode: \
             default layouts, no user workflows, nothing saved. Restart to leave safe mode."
        ));
    }
    let dirs = if safe_mode { None } else { found };
    let mut registry = Registry::load(dirs.as_ref(), PANEL_KINDS);

    let mut state = None;
    if let Some(dirs) = &dirs {
        let path = dirs.state_file();
        match StateFile::load(&path) {
            Ok(loaded) => state = loaded,
            Err(err) => {
                eprintln!("{err}");
                StateFile::set_aside(&path);
                notices.push(
                    "Saved workflow layouts could not be read and were set aside \
                     (state.json.bad); every workflow starts with its default layout."
                        .into(),
                );
            }
        }
        // Built-ins are never quarantined (they lead to safe mode above).
        if let Some(def) = unfinished
            .filter(|id| !id.is_builtin())
            .and_then(|id| registry.get(&id))
        {
            let state = state.get_or_insert_with(StateFile::default);
            state
                .quarantined
                .insert(def.id.clone(), "Carbon stopped while opening it".into());
            if let Err(err) = state.save(&path) {
                eprintln!("{err}");
            }
            notices.push(format!(
                "Workflow \"{}\" was turned off: Carbon stopped while opening it last time.",
                def.name
            ));
        }
    }

    if let Some(state) = &state {
        block(&mut registry, state);
    }
    log(&registry);
    if registry.uses_editor_fallback() {
        notices.push(
            "Carbon's workflow packages were not found, so only the Editor is available. \
             Reinstall Carbon or set CARBON_SHIPPED_WORKFLOWS_DIR."
                .into(),
        );
    }
    if !registry.rejected().is_empty() {
        let names: Vec<&str> = registry
            .rejected()
            .iter()
            .map(|rejected| rejected.source.as_str())
            .collect();
        notices.push(format!(
            "Some workflow files did not load: {}. Details are in the log.",
            names.join(", ")
        ));
    }
    Workflows {
        registry: RwLock::new(Arc::new(registry)),
        dirs,
        notices,
        state: Mutex::new(state),
    }
}

fn block(registry: &mut Registry, state: &StateFile) {
    registry.block(state.disabled.iter().cloned());
    registry.block(state.quarantined.keys().cloned());
    registry.block(state.changed.keys().cloned());
}

fn log(registry: &Registry) {
    for rejected in registry.rejected() {
        eprintln!(
            "workflow {} not loaded: {}",
            rejected.source,
            rejected.problems.join("; ")
        );
    }
    for (source, warning) in registry.warnings() {
        eprintln!("workflow {source}: {warning}");
    }
}

/// The current registry (a snapshot: keep it only as long as needed).
pub fn registry() -> Arc<Registry> {
    get()
        .registry
        .read()
        .map(|registry| registry.clone())
        .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
}

/// Reads the workflow files again and blocks what `state` turns off. Safe
/// mode keeps Carbon's own workflows only.
pub fn reload(state: &StateFile) {
    let mut registry = Registry::load(dirs(), PANEL_KINDS);
    block(&mut registry, state);
    log(&registry);
    let registry = Arc::new(registry);
    match get().registry.write() {
        Ok(mut slot) => *slot = registry,
        Err(poisoned) => *poisoned.into_inner() = registry,
    }
}

/// The user workflow folder; `None` in safe mode (`--safe-mode` or after an
/// unfinished start) or when the OS has none.
pub fn dirs() -> Option<&'static WorkflowDirs> {
    get().dirs.as_ref()
}

/// Messages for the user from loading (shown once).
pub fn notices() -> &'static [String] {
    &get().notices
}

/// `state.json` as read at start, once.
pub fn take_state() -> Option<StateFile> {
    get().state.lock().ok()?.take()
}

/// The workflow's Lucide icon if the app embeds it, else a generic one.
pub fn icon(def: &WorkflowDef) -> Icon {
    named_icon(def.icon.as_deref()).unwrap_or_else(|| Icon::new(IconName::LayoutDashboard))
}

/// A Lucide icon by name (`file-plus`) if the app embeds it.
pub fn named_icon(name: Option<&str>) -> Option<Icon> {
    let path = format!("icons/{}.svg", name?);
    matches!(AppAssets.load(&path), Ok(Some(_))).then(|| Icon::empty().path(path))
}

/// A workflow command ready for the menus and the palette.
pub struct WorkflowCommand {
    /// The workflow's name (palette group).
    pub group: SharedString,
    pub label: SharedString,
    /// Top-level menus it is placed in; none = palette only.
    pub menus: Vec<SharedString>,
    pub action: Box<dyn Action>,
}

impl Clone for WorkflowCommand {
    fn clone(&self) -> Self {
        Self {
            group: self.group.clone(),
            label: self.label.clone(),
            menus: self.menus.clone(),
            action: self.action.boxed_clone(),
        }
    }
}

/// The active workflow's commands. One whose action does not exist (or
/// rejects its arguments) is left out and logged once.
pub fn commands(id: &WorkflowId, cx: &App) -> Vec<WorkflowCommand> {
    static LOGGED: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    let registry = registry();
    let Some(def) = registry.get(id) else {
        return Vec::new();
    };
    def.commands
        .iter()
        .filter_map(|command| match build_action(command, cx) {
            Ok(action) => Some(WorkflowCommand {
                group: def.name.clone().into(),
                label: command.label.clone().into(),
                menus: def
                    .menus
                    .iter()
                    .filter(|item| item.command == command.id)
                    .map(|item| item.menu.clone().into())
                    .collect(),
                action,
            }),
            Err(err) => {
                let key = format!("{id}/{}", command.id);
                if let Ok(mut logged) = LOGGED.lock()
                    && logged.get_or_insert_with(HashSet::new).insert(key)
                {
                    eprintln!("workflow {id}: command `{}` left out: {err}", command.id);
                }
                None
            }
        })
        .collect()
}

/// The action of workflow `id`'s command `command` (a view's button or list
/// row), if it exists and builds.
pub fn command_action(id: &WorkflowId, command: &str, cx: &App) -> Option<Box<dyn Action>> {
    let registry = registry();
    let command = registry
        .get(id)?
        .commands
        .iter()
        .find(|c| c.id == command)?;
    build_action(command, cx).ok()
}

fn build_action(command: &CommandDef, cx: &App) -> Result<Box<dyn Action>, ActionBuildError> {
    let args = command
        .args
        .as_ref()
        .and_then(|args| serde_json::to_value(args).ok());
    cx.build_action(&command.run, args)
}
