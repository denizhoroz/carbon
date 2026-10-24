//! Extensions in the GUI: the loaded registry as a global, and applying what
//! the active extensions contribute: grammars (`grammars.rs`), themes
//! (`style::theme`), settings, and the language server table in the core.
//! Install and the Manage Extensions dialog come later
//! (`.agents/documents/extensions.md`).

mod code;
mod grammars;
pub mod host;
mod install;
mod manage;
mod tools;

use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::sync::Arc;

use carbon_core::Core;
use carbon_core::extension::{
    ExtensionDirs, ExtensionId, Origin, Registry, StateFile, verify_installed,
};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::*;

use crate::extension_panel::ADD_PREFIX;
use crate::menus::AddPanel;

pub use code::{
    ExtensionCode, check_graph, context as host_context, render_view, runs as code_runs,
    set_workspace as set_code_workspace,
};
pub use grammars::{PLAIN, highlight_language};
pub use manage::{Tab, open as open_manage};
pub use tools::ask_download;

/// The extension folders, the state file and what loaded from them.
pub struct Extensions {
    dirs: Option<ExtensionDirs>,
    state: StateFile,
    registry: Arc<Registry>,
    /// Extensions suggested this session (each at most once).
    suggested: HashSet<ExtensionId>,
}

impl Global for Extensions {}

/// Loads the extensions and applies them. Call after `settings::init` and
/// before `style::theme::init` (themes read the registry).
pub fn init(cx: &mut App) {
    let dirs = ExtensionDirs::from_env();
    let (state, problem) = match &dirs {
        Some(dirs) => StateFile::load_or_default(&dirs.state_file()),
        None => (StateFile::default(), None),
    };
    if let Some(problem) = problem {
        eprintln!("extensions: {problem}; starting with none turned off");
    }
    let registry = Arc::new(Registry::load(dirs.as_ref(), &state));
    report(&registry);
    set_snapshot(&registry);
    code::init(cx);
    cx.set_global(Extensions {
        dirs,
        state,
        registry: registry.clone(),
        suggested: HashSet::new(),
    });
    apply(&registry, cx);
    host::init(cx);
    check_installed(cx);
}

/// Checks installed extensions' files against the hashes recorded at
/// install, off the UI thread; changed ones are turned off until Reinstall
/// or Keep Changes (Manage Extensions).
fn check_installed(cx: &mut App) {
    let state = state(cx).clone();
    let packages: Vec<(ExtensionId, std::path::PathBuf, String)> = registry(cx)
        .all()
        .iter()
        .filter(|loaded| loaded.origin == Origin::Installed)
        .filter(|loaded| !state.changed.contains_key(&loaded.def.id))
        .filter_map(|loaded| {
            let record = state.installs.get(&loaded.def.id)?;
            Some((
                loaded.def.id.clone(),
                loaded.dir.clone(),
                record.hash.clone(),
            ))
        })
        .collect();
    if packages.is_empty() {
        return;
    }
    let check = cx.background_spawn(async move {
        packages
            .into_iter()
            .filter_map(|(id, dir, hash)| verify_installed(&dir, &hash).err().map(|why| (id, why)))
            .collect::<Vec<_>>()
    });
    cx.spawn(async move |cx| {
        let changed = check.await;
        if changed.is_empty() {
            return;
        }
        cx.update(|cx| {
            _ = update_state(cx, |state| {
                for (id, why) in changed {
                    state.changed.insert(id, why);
                }
            });
        });
    })
    .detach();
}

/// On a first start with no extensions: a note that languages and themes
/// come from extensions, with a button to the catalog. Shown once.
pub fn welcome(window: &mut Window, cx: &mut App) {
    let state = state(cx);
    if state.welcomed || !state.installs.is_empty() || !state.dev.is_empty() || dirs(cx).is_none() {
        return;
    }
    _ = update_state(cx, |state| state.welcomed = true);
    window.push_notification(
        Notification::new()
            .title("Carbon starts light")
            .message(
                "Languages, language servers and themes are extensions. Add the ones you \
                 need from the catalog.",
            )
            .action(|_, _, cx| {
                let note = cx.entity();
                Button::new("welcome-extensions")
                    .primary()
                    .label("Open Extensions")
                    .on_click(move |_, window, cx| {
                        note.update(cx, |note, cx| note.dismiss(window, cx));
                        open_manage(Tab::Catalog, window, cx)
                    })
            }),
        cx,
    );
}

/// When `path` has no language and the catalog lists an extension for it,
/// offers to install that extension (once per extension and session).
pub fn suggest(path: &Path, window: &mut Window, cx: &mut App) {
    if registry(cx).language_for_path(path).is_some() {
        return;
    }
    let Some(listed) = install::listed_for(path, cx) else {
        return;
    };
    if !cx
        .global_mut::<Extensions>()
        .suggested
        .insert(listed.id.clone())
    {
        return;
    }
    let message = if listed.description.is_empty() {
        "Carbon has no support for this kind of file yet.".to_string()
    } else {
        listed.description.clone()
    };
    let id = listed.id.clone();
    window.push_notification(
        Notification::new()
            .title(format!("Install {}?", listed.name))
            .message(message)
            .action(move |_, _, cx| {
                let (id, note) = (id.clone(), cx.entity());
                Button::new("suggest-install")
                    .primary()
                    .label("Install")
                    .on_click(move |_, window, cx| {
                        note.update(cx, |note, cx| note.dismiss(window, cx));
                        install::install_listed(&id, None, window, cx)
                    })
            }),
        cx,
    );
}

/// The loaded extensions as of the last (re)load, for code without the app
/// context (a webview's page protocol runs on its own thread).
static SNAPSHOT: std::sync::RwLock<Option<Arc<Registry>>> = std::sync::RwLock::new(None);

#[cfg_attr(not(feature = "webview"), allow(dead_code))]
pub fn registry_snapshot() -> Arc<Registry> {
    SNAPSHOT
        .read()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_default()
}

fn set_snapshot(registry: &Arc<Registry>) {
    if let Ok(mut slot) = SNAPSHOT.write() {
        *slot = Some(registry.clone());
    }
}

/// The loaded extensions.
pub fn registry(cx: &App) -> Arc<Registry> {
    cx.global::<Extensions>().registry.clone()
}

pub fn dirs(cx: &App) -> Option<&ExtensionDirs> {
    cx.global::<Extensions>().dirs.as_ref()
}

pub fn state(cx: &App) -> &StateFile {
    &cx.global::<Extensions>().state
}

/// Changes the state file (turn off, grant, link a folder), saves it and
/// reloads. The error says why it could not be saved; nothing changes then.
pub fn update_state(cx: &mut App, change: impl FnOnce(&mut StateFile)) -> Result<(), String> {
    let mut state = cx.global::<Extensions>().state.clone();
    change(&mut state);
    if let Some(dirs) = dirs(cx) {
        state.save(&dirs.state_file())?;
    }
    cx.global_mut::<Extensions>().state = state;
    reload(cx);
    Ok(())
}

/// Reads the folders again (after an install or removal) and applies what
/// the extensions now contribute. Observers of `Extensions` run after.
pub fn reload(cx: &mut App) {
    let this = cx.global::<Extensions>();
    let registry = Arc::new(Registry::load(this.dirs.as_ref(), &this.state));
    report(&registry);
    set_snapshot(&registry);
    code::unload_all(cx);
    cx.update_global::<Extensions, _>(|this, _| this.registry = registry.clone());
    apply(&registry, cx);
}

/// Problems found while loading, for the log until Manage Extensions shows them.
fn report(registry: &Registry) {
    for rejected in registry.rejected() {
        eprintln!(
            "extension not loaded: {}: {}",
            rejected.source,
            rejected.problems.join("; ")
        );
    }
    for (source, warning) in registry.warnings() {
        eprintln!("extension: {source}: {warning}");
    }
}

fn apply(registry: &Registry, cx: &mut App) {
    grammars::sync(registry);
    if cx.has_global::<host::AgentsHost>() {
        host::sync_agents(cx);
    }
    let active: BTreeSet<String> = registry
        .active()
        .map(|loaded| loaded.def.id.to_string())
        .collect();
    let contributed = registry.settings();
    crate::settings::update(cx, |store| {
        store.retain(|setting| {
            setting
                .extension
                .as_ref()
                .is_none_or(|id| active.contains(id))
        });
        for setting in contributed {
            store.register(setting);
        }
    });
    crate::style::theme::reload_themes(cx);
}

/// Whether extension `id` is active and has code.
pub fn has_code(id: &str, cx: &App) -> bool {
    let registry = registry(cx);
    let id = carbon_core::extension::ExtensionId::new(id);
    registry.is_active(&id)
        && registry
            .get(&id)
            .is_some_and(|loaded| loaded.def.wasm.is_some())
}

/// `config` with the command the extension's code gives for it, if any and
/// allowed (`process.exec:<program>`); `config` unchanged otherwise.
pub fn language_server_command(
    config: &Arc<carbon_core::lsp::ServerConfig>,
    cx: &mut App,
) -> Task<Arc<carbon_core::lsp::ServerConfig>> {
    let id = carbon_core::extension::ExtensionId::new(&config.extension);
    let registry = registry(cx);
    let allowed: BTreeSet<String> = match registry.get(&id) {
        Some(loaded) if loaded.origin == Origin::Dev => {
            loaded.def.capabilities.iter().cloned().collect()
        }
        Some(_) => state(cx).grants.get(&id).cloned().unwrap_or_default(),
        None => BTreeSet::new(),
    };
    let ask = code::language_server_command(id, config.server_id.clone(), cx);
    let config = config.clone();
    cx.spawn(async move |_| {
        let command = match ask.await {
            Ok(Some(command)) => command,
            Ok(None) => return config,
            Err(err) => {
                eprintln!("{}: language server command: {err}", config.extension);
                return config;
            }
        };
        let path = Path::new(&command.program);
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or(&command.program);
        if !allowed.contains("process.exec:*") && !allowed.contains(&format!("process.exec:{name}"))
        {
            eprintln!(
                "{}: may not run {} (needs process.exec:{name})",
                config.extension, command.program
            );
            return config;
        }
        let program = if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            carbon_core::lsp::find_program(&command.program)
        };
        let mut changed = (*config).clone();
        changed.command = name.to_string();
        changed.program = program;
        changed.args = command.args;
        changed.env = command.env.into_iter().collect();
        Arc::new(changed)
    })
}

/// Runs extension command `key` (`<extension id>/<command id>`): its host
/// action, its panel, or its code.
pub fn run_command(key: &str, window: &mut Window, cx: &mut App) {
    let registry = registry(cx);
    let Some((loaded, command)) = registry.command(key) else {
        window.push_notification(format!("Command `{key}` is not available"), cx);
        return;
    };
    let id = loaded.def.id.clone();
    if !registry.is_active(&id) {
        window.push_notification(format!("{} is turned off", loaded.def.name), cx);
        return;
    }
    if let Some(action) = &command.action {
        let args = command
            .args
            .as_ref()
            .and_then(|args| serde_json::to_value(args).ok());
        match cx.build_action(action, args) {
            Ok(action) => window.dispatch_action(action, cx),
            Err(err) => window.push_notification(format!("{}: {err}", command.title), cx),
        }
    } else if let Some(panel) = &command.panel {
        let name = format!("{ADD_PREFIX}{id}/{panel}");
        window.dispatch_action(Box::new(AddPanel { name: name.into() }), cx);
    } else {
        let title = command.title.clone();
        let task = code::run_command(id, command.id.clone(), cx);
        let window = window.window_handle();
        cx.spawn(async move |cx| {
            if let Err(err) = task.await {
                _ = window.update(cx, |_, window, cx| {
                    window.push_notification(format!("{title}: {err}"), cx)
                });
            }
        })
        .detach();
    }
}

/// Extension menu items and panels for the menus, from the active extensions.
pub fn menu_items(
    cx: &App,
) -> (
    Vec<crate::menus::ExtensionItem>,
    Vec<crate::menus::ExtensionItem>,
) {
    let registry = registry(cx);
    let items = registry
        .active()
        .flat_map(|loaded| {
            loaded.def.menus.iter().filter_map(|item| {
                let command = loaded.def.commands.iter().find(|c| c.id == item.command)?;
                Some(crate::menus::ExtensionItem {
                    menu: item.menu.clone().into(),
                    title: command.title.clone().into(),
                    key: format!("{}/{}", loaded.def.id, command.id).into(),
                })
            })
        })
        .collect();
    let panels = registry
        .panels()
        .into_iter()
        .filter(|(_, panel)| !panel.explorer_section)
        .map(|(loaded, panel)| crate::menus::ExtensionItem {
            menu: "View".into(),
            title: panel.title.clone().into(),
            key: format!("{ADD_PREFIX}{}/{}", loaded.def.id, panel.id).into(),
        })
        .collect();
    (items, panels)
}

/// Every extension command for the palette: (group, title, keywords, key).
pub fn palette_commands(
    cx: &App,
) -> Vec<(SharedString, SharedString, Vec<SharedString>, SharedString)> {
    let registry = registry(cx);
    registry
        .commands()
        .into_iter()
        .map(|(loaded, command)| {
            (
                loaded.def.name.clone().into(),
                command.title.clone().into(),
                command
                    .keywords
                    .iter()
                    .cloned()
                    .map(SharedString::from)
                    .collect(),
                format!("{}/{}", loaded.def.id, command.id).into(),
            )
        })
        .collect()
}

/// The language server table for the core, from the active extensions.
pub fn sync_core(core: &mut Core, cx: &App) {
    let table = registry(cx).server_table(dirs(cx), |_, _| None);
    if core.lsp.table() != &table {
        core.lsp.set_table(table);
    }
}
