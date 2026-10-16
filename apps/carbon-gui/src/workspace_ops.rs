//! Opening, creating and closing the workspace folder. Reached from the File
//! menu, the status bar workspace button and the Explorer empty state (all via
//! the `OpenFolder` / `CreateFolder` / `CloseFolder` actions).

use std::path::{Path, PathBuf};

use carbon_core::Core;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
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
        dialog
            .title("Create Folder")
            .w(fixed::DIALOG_WIDTH)
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
                                submit.update(cx, |form, cx| form.create(window, cx))
                            }),
                    ),
            )
    });
    name_input.update(cx, |input, cx| input.focus(window, cx));
}

struct CreateFolderForm {
    core: Entity<Core>,
    name: Entity<InputState>,
    location: PathBuf,
    _enter: Subscription,
}

impl CreateFolderForm {
    fn new(
        core: Entity<Core>,
        location: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("my-project"));
        let _enter = cx.subscribe_in(&name, window, |this, _, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.create(window, cx);
            }
        });
        Self {
            core,
            name,
            location,
            _enter,
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

    /// Creates and opens the folder; on error keeps the dialog open and says why.
    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().to_string();
        let location = self.location.clone();
        let result = self.core.update(cx, |core, cx| {
            let result = core.create_workspace(&location, &name).map(|_| ());
            cx.notify();
            result
        });
        match result {
            Ok(()) => window.close_dialog(cx),
            Err(err) => window.push_notification(format!("Cannot create folder: {err}"), cx),
        }
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
                            .truncate()
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
