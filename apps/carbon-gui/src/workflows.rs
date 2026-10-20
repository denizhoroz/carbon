//! The workflow registry as the GUI sees it, loaded once at start
//! (`project/not_built/workflow-system.md`): built-ins, user workflows and
//! overlays from `WorkflowDirs`, with disabled and quarantined ones blocked.
//! Also: the crash-loop guard at start, safe mode, the panel kinds manifests
//! may name, workflow icons and workflow commands.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use carbon_core::workflow::{
    Registry, StateFile, WorkflowDef, WorkflowDirs, WorkflowId, take_unfinished,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{Action, App, AssetSource, SharedString};

use crate::app_assets::AppAssets;

/// Panel kinds a manifest may name; `layouts::PanelKind` builds them.
pub const PANEL_KINDS: [&str; 5] = ["editor", "explorer", "terminal", "chat", "placeholder"];

pub struct Workflows {
    registry: Registry,
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
        registry: Registry::builtin(&PANEL_KINDS),
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
        let builtins = Registry::builtin(&PANEL_KINDS);
        let name = builtins
            .get(id)
            .map_or(id.as_str(), |def| def.name.as_str());
        notices.push(format!(
            "Carbon stopped while opening \"{name}\" last time, so it started in safe mode: \
             default layouts, no user workflows, nothing saved. Restart to leave safe mode."
        ));
    }
    let dirs = if safe_mode { None } else { found };
    let mut registry = Registry::load(dirs.as_ref(), &PANEL_KINDS);

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
        registry.block(state.disabled.iter().cloned());
        registry.block(state.quarantined.keys().cloned());
    }
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
        registry,
        dirs,
        notices,
        state: Mutex::new(state),
    }
}

pub fn registry() -> &'static Registry {
    &get().registry
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
    let path = def.icon.as_ref().map(|name| format!("icons/{name}.svg"));
    match path {
        Some(path) if matches!(AppAssets.load(&path), Ok(Some(_))) => Icon::empty().path(path),
        _ => Icon::new(IconName::LayoutDashboard),
    }
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
    let Some(def) = registry().get(id) else {
        return Vec::new();
    };
    def.commands
        .iter()
        .filter_map(|command| {
            let args = command
                .args
                .as_ref()
                .and_then(|args| serde_json::to_value(args).ok());
            match cx.build_action(&command.run, args) {
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
            }
        })
        .collect()
}
