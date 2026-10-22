//! Settings: the app-wide store (`carbon_core::settings::Settings`) as a GPUI
//! global, the list of Carbon's own settings, and the Settings dialog
//! (`view.rs`). Features read values with `settings::get(cx)` and observe
//! the `AppSettings` global to follow changes; files edited elsewhere (the
//! JSON tab, another program) are noticed by polling.

mod row;
mod tree;
mod view;

use std::path::{Path, PathBuf};
use std::time::Duration;

use carbon_core::settings::{Scope, Settings, USER_FILE, user_config_dir};
use gpui_kit::component::WindowExt;
use gpui_kit::*;
use serde_json::Value;

pub use view::open_dialog;

/// How often the settings files are checked for edits made elsewhere (two
/// `stat`s).
const POLL: Duration = Duration::from_secs(1);

/// The store, plus a counter that moves on every change (views compare it to
/// know whether to re-apply).
pub struct AppSettings {
    store: Settings,
    generation: u64,
}

impl Global for AppSettings {}

/// Creates the store with Carbon's own settings and starts watching the
/// files. Call before anything reads settings (`style::theme::init`).
pub fn init(cx: &mut App) {
    let dir = user_config_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut store = Settings::new(dir.join(USER_FILE));
    for setting in crate::style::theme::settings()
        .into_iter()
        .chain(crate::editor_panel::settings())
        .chain(crate::explorer_panel::settings())
    {
        store.register(setting);
    }
    cx.set_global(AppSettings {
        store,
        generation: 0,
    });
    view::register(cx);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL).await;
            let changed = cx.update(|cx| get(cx).files_changed());
            if changed {
                cx.update(|cx| update(cx, Settings::reload_changed));
            }
        }
    })
    .detach();
}

pub fn get(cx: &App) -> &Settings {
    &cx.global::<AppSettings>().store
}

/// Moves on every change to the settings.
pub fn generation(cx: &App) -> u64 {
    cx.global::<AppSettings>().generation
}

/// Changes the store; observers of `AppSettings` run after.
pub fn update<R>(cx: &mut App, f: impl FnOnce(&mut Settings) -> R) -> R {
    cx.update_global::<AppSettings, _>(|settings, _| {
        settings.generation += 1;
        f(&mut settings.store)
    })
}

/// Writes a value; a failure (a file with errors) shows as a notification.
pub fn set(scope: Scope, key: &str, value: Value, window: &mut Window, cx: &mut App) {
    if get(cx).value_in(scope, key) == Some(&value) {
        return;
    }
    if let Err(err) = update(cx, |store| store.set(scope, key, value)) {
        window.push_notification(format!("Cannot change setting: {err}"), cx);
    }
}

/// Removes a value from `scope`, back to the next scope or the default.
pub fn reset(scope: Scope, key: &str, window: &mut Window, cx: &mut App) {
    if let Err(err) = update(cx, |store| store.reset(scope, key)) {
        window.push_notification(format!("Cannot reset setting: {err}"), cx);
    }
}

/// Points the workspace scope at the open folder (or none). Call when the
/// core's workspace may have changed.
pub fn sync_workspace(root: Option<&Path>, cx: &mut App) {
    let wanted = root.map(|root| root.join(carbon_core::settings::WORKSPACE_FILE));
    if get(cx).path(Scope::Workspace) != wanted.as_deref() {
        update(cx, |store| store.set_workspace(root));
    }
}

/// Opens `scope`'s JSON file in an editor tab (created empty if missing).
pub fn open_file(scope: Scope, window: &mut Window, cx: &mut App) {
    let path = match update(cx, |store| store.ensure_file(scope)) {
        Ok(path) => path,
        Err(err) => {
            window.push_notification(format!("Cannot open settings file: {err}"), cx);
            return;
        }
    };
    let core = crate::workspace_ops::current_core(cx);
    let result = core.update(cx, |core, cx| {
        let result = core.open_any_file(&path);
        cx.notify();
        result
    });
    match result {
        Ok(id) => crate::workbench::open_buffer(id, window, cx),
        Err(err) => window.push_notification(format!("Cannot open settings file: {err}"), cx),
    }
}

/// Whether `path` is one of the settings files (editable even outside the
/// open folder).
pub fn is_settings_file(path: &Path, cx: &App) -> bool {
    get(cx).is_settings_file(path)
}
