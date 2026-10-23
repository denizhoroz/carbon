//! Starting language servers and following what they report. The core holds
//! the servers and their state; this drives the async parts on the app's
//! executor: initializing a new server, then reading its notifications
//! (diagnostics, progress) until it stops.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use carbon_core::Core;
use carbon_core::lsp::{
    self, LanguageServer, LspStatus, ServerConfig, ServerNotification, lsp_types,
};
use gpui_kit::*;

/// Languages whose server command is being asked of extension code.
#[derive(Default)]
struct Asking(HashSet<String>);

impl Global for Asking {}

/// Starts the server for `path`'s language unless the workspace has one
/// (or tried to start one) already. When the server's extension has code,
/// the code may give the command first (`language-server-command`).
pub fn ensure_started(core: &Entity<Core>, path: &Path, cx: &mut App) {
    let Some(config) = core.read(cx).lsp.server_for_path(path) else {
        return;
    };
    if core.read(cx).lsp.entry(&config.language).is_some() {
        return;
    }
    if !crate::extensions::has_code(&config.extension, cx) {
        start(core, config, cx);
        return;
    }
    if !cx
        .default_global::<Asking>()
        .0
        .insert(config.language.clone())
    {
        return;
    }
    let task = crate::extensions::language_server_command(&config, cx);
    let core = core.clone();
    cx.spawn(async move |cx| {
        let config = task.await;
        cx.update(|cx| {
            cx.default_global::<Asking>().0.remove(&config.language);
            start(&core, config, cx);
        });
    })
    .detach();
}

/// Starts `config`'s server in the open folder and follows it.
fn start(core: &Entity<Core>, config: Arc<ServerConfig>, cx: &mut App) {
    let language = config.language.clone();
    let started = core.update(cx, |core, cx| {
        let root = core.workspace()?.root().to_path_buf();
        let server = core.lsp.start(config, &root);
        cx.notify();
        Some((server?, root))
    });
    let Some((server, root)) = started else {
        return;
    };
    follow(core.downgrade(), language.clone(), &server, cx);

    let core = core.downgrade();
    let initialize = lsp::initialize(server.clone(), &root);
    cx.spawn(async move |cx| {
        let result = initialize.await;
        _ = core.update(cx, |core, cx| {
            if !is_current(core, &language, &server) {
                return;
            }
            match result {
                Ok(_) => {
                    core.lsp.set_status(&language, LspStatus::Ready);
                    core.lsp_open_buffers();
                }
                Err(err) => core.lsp.set_status(
                    &language,
                    LspStatus::Failed(failure(&err.to_string(), &server)),
                ),
            }
            cx.notify();
        });
    })
    .detach();
}

/// Whether `server` is still the workspace's server for `language` (the
/// workspace may have closed meanwhile, with a new store).
fn is_current(core: &Core, language: &str, server: &Arc<LanguageServer>) -> bool {
    core.lsp
        .entry(language)
        .and_then(|entry| entry.server())
        .is_some_and(|current| Arc::ptr_eq(current, server))
}

fn failure(error: &str, server: &LanguageServer) -> String {
    match server.stderr_tail() {
        tail if tail.is_empty() => error.to_string(),
        tail => format!("{error}\n{tail}"),
    }
}

/// Reads the server's notifications until it stops. Holds no strong
/// reference, so closing the workspace stops the server and ends this.
fn follow(core: WeakEntity<Core>, language: String, server: &Arc<LanguageServer>, cx: &mut App) {
    let notifications = server.notifications();
    let server = Arc::downgrade(server);
    cx.spawn(async move |cx| {
        while let Ok(message) = notifications.recv().await {
            if core
                .update(cx, |core, cx| handle(core, &language, message, cx))
                .is_err()
            {
                return;
            }
        }
        // The stream ended while the store still holds the server: it crashed.
        let Some(server) = server.upgrade() else {
            return;
        };
        _ = core.update(cx, |core, cx| {
            if is_current(core, &language, &server) {
                core.lsp
                    .set_status(&language, LspStatus::Failed(failure("stopped", &server)));
                cx.notify();
            }
        });
    })
    .detach();
}

fn handle(core: &mut Core, language: &str, message: ServerNotification, cx: &mut Context<Core>) {
    match message.method.as_str() {
        "textDocument/publishDiagnostics" => {
            let Ok(params) =
                serde_json::from_value::<lsp_types::PublishDiagnosticsParams>(message.params)
            else {
                return;
            };
            if let Some(path) = lsp::uri_to_path(&params.uri) {
                core.set_diagnostics(path, params.diagnostics);
                cx.notify();
            }
        }
        "$/progress" => {
            let Ok(params) = serde_json::from_value::<lsp_types::ProgressParams>(message.params)
            else {
                return;
            };
            if progress(core, language, params) {
                cx.notify();
            }
        }
        _ => {}
    }
}

/// Tracks work the server reports; the status names one running piece.
/// Returns whether the status changed.
fn progress(core: &mut Core, language: &str, params: lsp_types::ProgressParams) -> bool {
    use lsp_types::{NumberOrString, ProgressParamsValue, WorkDoneProgress};
    let Some(entry) = core.lsp.entry_mut(language) else {
        return false;
    };
    let token = match params.token {
        NumberOrString::Number(number) => number.to_string(),
        NumberOrString::String(text) => text,
    };
    let ProgressParamsValue::WorkDone(value) = params.value;
    match value {
        WorkDoneProgress::Begin(begin) => {
            entry.progress.insert(token, begin.title);
        }
        WorkDoneProgress::End(_) => {
            entry.progress.remove(&token);
        }
        WorkDoneProgress::Report(_) => return false,
    }
    if !matches!(entry.status, LspStatus::Ready | LspStatus::Busy(_)) {
        return false;
    }
    let status = match entry.progress.values().next() {
        Some(title) => LspStatus::Busy(title.clone()),
        None => LspStatus::Ready,
    };
    let changed = entry.status != status;
    entry.status = status;
    changed
}
