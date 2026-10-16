//! Opening, creating and closing the workspace folder. Reached from the File
//! menu, the status bar workspace button and the Explorer empty state (all via
//! the `OpenFolder` / `CreateFolder` / `CloseFolder` actions).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use carbon_core::{Core, FsChanges, git_status};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, WindowExt, h_flex, v_flex};
use gpui_kit::*;

use crate::style::fixed;

/// The window's core, for panels the layout registry builds (it only hands
/// them `App`). One window for now; more windows need a per-window lookup.
pub struct CurrentCore(pub Entity<Core>);

impl Global for CurrentCore {}

pub fn current_core(cx: &App) -> Entity<Core> {
    cx.global::<CurrentCore>().0.clone()
}

fn folder_prompt(title: &str) -> PathPromptOptions {
    PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(title.to_string().into()),
    }
}

/// Native folder picker, then opens the chosen folder.
pub fn open_folder(core: Entity<Core>, window: &mut Window, cx: &mut App) {
    let picked = cx.prompt_for_paths(folder_prompt("Open Folder"));
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            _ = cx.update(|window, cx| {
                let result = core.update(cx, |core, cx| {
                    let result = core.open_workspace(&path).map(|_| ());
                    cx.notify();
                    result
                });
                if let Err(err) = result {
                    window.push_notification(format!("Cannot open folder: {err}"), cx);
                }
            });
        })
        .detach();
}

/// Least time between two git status runs: a build writing thousands of
/// files must not keep a status run going all the time.
const GIT_STATUS_INTERVAL: Duration = Duration::from_secs(1);

/// Applies the core's file system changes (the explorer tree re-lists the
/// folders they name) and keeps the git status current, until the channel
/// closes with its watcher. Git status runs off the UI thread: once at the
/// start, then after changes that may affect it, at most once per interval.
pub fn apply_changes(
    core: &Entity<Core>,
    changes: async_channel::Receiver<FsChanges>,
    cx: &mut App,
) -> Task<()> {
    let root = core.read(cx).workspace().map(|w| w.root().to_path_buf());
    let core = core.downgrade();
    cx.spawn(async move |cx| {
        let Some(root) = root else {
            return;
        };
        if !refresh_git(&core, &root, cx).await {
            return;
        }
        let mut last_git = Instant::now();
        while let Ok(batch) = changes.recv().await {
            let mut git = batch.git;
            let mut batches = vec![batch];
            if git {
                // Wait out the interval, gathering what arrives meanwhile.
                let wait = GIT_STATUS_INTERVAL.saturating_sub(last_git.elapsed());
                if !wait.is_zero() {
                    cx.background_executor().timer(wait).await;
                }
            }
            while let Ok(more) = changes.try_recv() {
                git |= more.git;
                batches.push(more);
            }
            let applied = core.update(cx, |core, cx| {
                let mut changed = false;
                for batch in &batches {
                    match core.apply_fs_changes(batch) {
                        Ok(true) => changed = true,
                        Ok(false) => {}
                        Err(err) => eprintln!("explorer refresh failed: {err}"),
                    }
                }
                if changed {
                    cx.notify();
                }
            });
            if applied.is_err() {
                break;
            }
            if git {
                if !refresh_git(&core, &root, cx).await {
                    break;
                }
                last_git = Instant::now();
            }
        }
    })
}

/// Runs `git_status` in the background and stores it; false once the core is gone.
async fn refresh_git(core: &WeakEntity<Core>, root: &Path, cx: &mut AsyncApp) -> bool {
    let dir = root.to_path_buf();
    let status = cx
        .background_executor()
        .spawn(async move { git_status(&dir) })
        .await;
    let status = status.unwrap_or_else(|err| {
        eprintln!("git status failed: {err}");
        None
    });
    core.update(cx, |core, cx| {
        if core.git() != status.as_ref() {
            core.set_git(status);
            cx.notify();
        }
    })
    .is_ok()
}

/// Test hook (debug builds): `CARBON_TEST_MARKS="src/main.rs:2:1:agent;docs:0:3"`
/// gives files problem counts (errors:warnings) and an agent mark, so the
/// Explorer markers can be seen before diagnostics and agents exist.
#[cfg(debug_assertions)]
pub fn seed_test_marks(core: &mut Core) {
    let Ok(spec) = std::env::var("CARBON_TEST_MARKS") else {
        return;
    };
    let Some(root) = core.workspace().map(|w| w.root().to_path_buf()) else {
        return;
    };
    for item in spec.split(';').filter(|item| !item.is_empty()) {
        let mut parts = item.split(':');
        let path = root.join(parts.next().unwrap_or_default());
        let errors = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let warnings = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        core.marks.set_problems(&path, errors, warnings);
        if parts.next() == Some("agent") {
            core.marks.set_agent_edited(&path, true);
        }
    }
}

pub fn close_folder(core: &Entity<Core>, cx: &mut App) {
    core.update(cx, |core, cx| {
        core.close_workspace();
        cx.notify();
    });
}

/// Where a new folder goes by default: next to the open workspace, else home.
fn default_location(core: &Core) -> PathBuf {
    core.workspace()
        .and_then(|w| w.root().parent().map(Path::to_path_buf))
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// Dialog: folder name + location (Browse...), then creates and opens it.
pub fn open_create_dialog(core: Entity<Core>, window: &mut Window, cx: &mut App) {
    let location = default_location(core.read(cx));
    let form = cx.new(|cx| CreateFolderForm::new(core, location, window, cx));
    let name_input = form.read(cx).name.clone();

    window.open_dialog(cx, move |dialog, _, _| {
        let submit = form.clone();
        let confirm = form.clone();
        dialog
            .title("Create Folder")
            .w(fixed::DIALOG_WIDTH)
            // Enter is the dialog's Confirm; returning false keeps it open.
            .on_ok(move |_, window, cx| confirm.update(cx, |form, cx| form.create(window, cx)))
            .child(form.clone())
            .footer(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("create-folder-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("create-folder-ok")
                            .primary()
                            .label("Create")
                            .on_click(move |_, window, cx| {
                                if submit.update(cx, |form, cx| form.create(window, cx)) {
                                    window.close_dialog(cx);
                                }
                            }),
                    ),
            )
    });
    // Deferred: a click that opened the dialog (e.g. from a panel) may still
    // move focus this frame.
    window.defer(cx, move |window, cx| {
        name_input.update(cx, |input, cx| input.focus(window, cx))
    });
}

struct CreateFolderForm {
    core: Entity<Core>,
    name: Entity<InputState>,
    location: PathBuf,
}

impl CreateFolderForm {
    fn new(
        core: Entity<Core>,
        location: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("my-project"));
        Self {
            core,
            name,
            location,
        }
    }

    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(folder_prompt("Choose Location"));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(path) = paths.into_iter().next()
            {
                _ = this.update(cx, |this, cx| {
                    this.location = path;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Creates and opens the folder. On error says why and returns false, so
    /// the caller keeps the dialog open.
    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let name = self.name.read(cx).value().to_string();
        let location = self.location.clone();
        let result = self.core.update(cx, |core, cx| {
            let result = core.create_workspace(&location, &name).map(|_| ());
            cx.notify();
            result
        });
        if let Err(err) = &result {
            window.push_notification(format!("Cannot create folder: {err}"), cx);
        }
        result.is_ok()
    }
}

impl Render for CreateFolderForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label = |text: &'static str| div().text_sm().child(text);
        v_flex()
            .gap(fixed::PANEL_GAP)
            .child(label("Name"))
            .child(Input::new(&self.name))
            .child(label("Location"))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            // Paths: keep the end (the folder) visible.
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis_start()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.location.display().to_string()),
                    )
                    .child(
                        Button::new("create-folder-browse")
                            .label("Browse...")
                            .on_click(cx.listener(|this, _, window, cx| this.browse(window, cx))),
                    ),
            )
    }
}
