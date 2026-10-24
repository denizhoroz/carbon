//! Downloading a language server an extension offers when it is not
//! installed: the status bar's "not installed" item asks first (what, from
//! where, checked how), then the download runs as a job with its progress in
//! the title, and the server starts once it is in place.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use carbon_core::Core;
use carbon_core::extension::{
    ExtensionId, Origin, ToolProgress, download_host, download_tool, tool_download,
};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use crate::style::fixed;

/// How often the job title shows the bytes received.
const PROGRESS_EVERY: Duration = Duration::from_millis(300);

/// Asks whether to download `language`'s server, then does it.
pub fn ask_download(core: Entity<Core>, language: String, window: &mut Window, cx: &mut App) {
    let Some(config) = core.read(cx).lsp.table().server(&language).cloned() else {
        return;
    };
    let registry = super::registry(cx);
    let Some(loaded) = registry.get(&ExtensionId::new(&config.extension)).cloned() else {
        return;
    };
    let Some(server) = loaded
        .def
        .language_servers
        .iter()
        .find(|server| server.id == config.server_id)
        .cloned()
    else {
        return;
    };
    let Some(download) = tool_download(&server).cloned() else {
        return;
    };
    let host = download_host(&download);
    // Linked development folders are trusted; installed ones need the grant.
    let granted =
        loaded.origin == Origin::Dev || super::state(cx).allows_net(&loaded.def.id, &host);
    if !granted {
        window.push_notification(
            format!(
                "{} may not connect to {host}: allow it in Manage Extensions first.",
                loaded.def.name
            ),
            cx,
        );
        return;
    }
    let Some(dirs) = super::dirs(cx).cloned() else {
        return;
    };
    let folder = dirs.tools_dir().display().to_string();
    let title = format!("Download {} {}?", server.name, download.version);
    let body = format!(
        "{} was not found on this computer. The {} extension can download version {} from \
         {host}; Carbon checks it against the SHA-256 in the extension before using it.",
        server.name, loaded.def.name, download.version
    );
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        let start = {
            let (core, language, loaded, server, dirs) = (
                core.clone(),
                language.clone(),
                loaded.clone(),
                server.clone(),
                dirs.clone(),
            );
            Rc::new(move |window: &mut Window, cx: &mut App| {
                run(
                    core.clone(),
                    language.clone(),
                    loaded.clone(),
                    server.clone(),
                    dirs.clone(),
                    window,
                    cx,
                )
            })
        };
        let on_ok = start.clone();
        dialog
            .title(title.clone())
            .w(fixed::DIALOG_WIDTH)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
            .child(
                v_flex().gap(fixed::PANEL_GAP).child(body.clone()).child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(format!("Saved in {folder}")),
                ),
            )
            .footer(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("tool-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("tool-download")
                            .primary()
                            .label("Download")
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                start(window, cx);
                            }),
                    ),
            )
    });
}

/// Downloads off the UI thread as a job; starts the server when done.
fn run(
    core: Entity<Core>,
    language: String,
    loaded: carbon_core::extension::Loaded,
    server: carbon_core::extension::LanguageServerDef,
    dirs: carbon_core::extension::ExtensionDirs,
    window: &mut Window,
    cx: &mut App,
) {
    let name = server.name.clone();
    let job = core.update(cx, |core, cx| {
        let job = core.jobs.start(format!("Downloading {name}"));
        cx.notify();
        job
    });
    let progress = Arc::new(ToolProgress::default());
    let download = cx.background_spawn({
        let progress = progress.clone();
        async move { download_tool(&dirs, &loaded, &server, &progress) }
    });
    let window_handle = window.window_handle();
    cx.spawn(async move |cx| {
        let mut download = std::pin::pin!(download);
        let result = loop {
            let tick = cx.background_executor().timer(PROGRESS_EVERY);
            match futures_select(&mut download, tick).await {
                Some(result) => break result,
                None => {
                    let (received, total) = progress.bytes();
                    let text = match total {
                        0 => format!("Downloading {name}: {} MB", mb(received)),
                        total => format!("Downloading {name}: {} / {} MB", mb(received), mb(total)),
                    };
                    core.update(cx, |core, cx| {
                        core.jobs.set_title(job, text);
                        cx.notify();
                    });
                }
            }
        };
        cx.update(|cx| {
            core.update(cx, |core, cx| {
                core.jobs
                    .finish(job, result.as_ref().map(|_| ()).map_err(Clone::clone));
                core.jobs.set_title(job, format!("Downloaded {name}"));
                if result.is_ok() {
                    // The table now finds the downloaded copy; start again.
                    super::sync_core(core, cx);
                    core.lsp.forget(&language);
                }
                cx.notify();
            });
            match &result {
                Ok(_) => {
                    let state = core.read(cx);
                    let paths: Vec<_> = state
                        .buffers
                        .ids()
                        .filter_map(|id| state.buffers.get(id)?.path().map(|p| p.to_path_buf()))
                        .filter(|path| {
                            state.lsp.table().language_for_path(path) == Some(language.as_str())
                        })
                        .collect();
                    for path in paths {
                        crate::lsp_ops::ensure_started(&core, &path, cx);
                    }
                }
                Err(err) => {
                    let message = format!("Cannot download {name}: {err}");
                    _ = window_handle.update(cx, |_, window, cx| {
                        window.push_notification(message, cx);
                    });
                }
            }
        });
    })
    .detach();
}

fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / (1024.0 * 1024.0))
}

/// Waits for `task` or `tick`, whichever ends first: `Some` with the task's
/// output, `None` when the tick came first.
async fn futures_select<T: 'static>(
    task: &mut std::pin::Pin<&mut Task<T>>,
    tick: Task<()>,
) -> Option<T> {
    use std::future::Future as _;
    use std::task::Poll;
    let mut tick = std::pin::pin!(tick);
    std::future::poll_fn(|context| {
        if let Poll::Ready(output) = task.as_mut().poll(context) {
            return Poll::Ready(Some(output));
        }
        if tick.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}
