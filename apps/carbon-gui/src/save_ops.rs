//! Saving buffers and quitting. Saving a buffer with no file asks where to
//! put it; quitting (Exit, Ctrl+Q, the window's x) with unsaved changes asks
//! Save All / Don't Save / Cancel first. Auto save comes later, with settings.

use std::path::PathBuf;
use std::rc::Rc;

use carbon_core::{BufferId, Core};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::style::fixed;
use crate::workbench::WindowCallback;
use crate::workspace_ops::current_core;

/// Files the quit dialog names before "and N more".
const QUIT_LIST_MAX: usize = 5;

/// Saves `ids` in order; a buffer with no file asks for a path first.
/// `done` gets true when every one was saved, false when the user cancelled
/// a path or a save failed (a notification says which).
pub fn save_buffers(
    ids: Vec<BufferId>,
    window: &mut Window,
    cx: &mut App,
    done: impl FnOnce(bool, &mut Window, &mut App) + 'static,
) {
    let core = current_core(cx);
    window
        .spawn(cx, async move |cx| {
            let mut saved = true;
            for id in ids {
                let Ok(buffer) = cx.update(|_, cx| {
                    let buffer = core.read(cx).buffers.get(id)?;
                    Some((buffer.name(), buffer.path().is_some()))
                }) else {
                    return;
                };
                // Closed meanwhile: nothing to save.
                let Some((name, has_path)) = buffer else {
                    continue;
                };
                let new_path = if has_path {
                    None
                } else {
                    let Ok(chosen) = cx.update(|_, cx| {
                        let dir = default_dir(core.read(cx));
                        cx.prompt_for_new_path(&dir, Some(&format!("{name}.txt")))
                    }) else {
                        return;
                    };
                    match chosen.await {
                        Ok(Ok(Some(path))) => Some(path),
                        Ok(Err(err)) => {
                            _ = cx.update(|window, cx| {
                                window.push_notification(format!("Cannot save {name}: {err}"), cx)
                            });
                            saved = false;
                            break;
                        }
                        // Cancelled.
                        _ => {
                            saved = false;
                            break;
                        }
                    }
                };
                let ok = cx.update(|window, cx| {
                    let result = core.update(cx, |core, cx| {
                        let result = match &new_path {
                            Some(path) => core.save_buffer_as(id, path),
                            None => core.save_buffer(id),
                        };
                        cx.notify();
                        result
                    });
                    if let Err(err) = &result {
                        window.push_notification(format!("Cannot save {name}: {err}"), cx);
                    }
                    result.is_ok()
                });
                if !matches!(ok, Ok(true)) {
                    saved = false;
                    break;
                }
            }
            _ = cx.update(|window, cx| done(saved, window, cx));
        })
        .detach();
}

/// Where the save dialog starts: the open folder, else the home folder.
fn default_dir(core: &Core) -> PathBuf {
    core.workspace()
        .map(|workspace| workspace.root().to_path_buf())
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// The Quit action (Exit, Ctrl+Q): asks about unsaved changes first.
pub fn quit(cx: &mut App) {
    let window = cx
        .active_window()
        .or_else(|| cx.windows().into_iter().next());
    match window {
        // Deferred: from a menu or key this runs inside the window's own
        // update, where the window cannot be updated again.
        Some(window) => cx.defer(move |cx| {
            if window
                .update(cx, |_, window, cx| request_quit(window, cx))
                .is_err()
            {
                cx.quit();
            }
        }),
        None => cx.quit(),
    }
}

/// Quits now, or after the unsaved-changes dialog. Running agent flows are
/// stopped, after asking.
pub fn request_quit(window: &mut Window, cx: &mut App) {
    let running = crate::extensions::host::running_runs(cx);
    if running > 0 {
        let text = if running == 1 {
            "An agent flow is running. Quitting stops it.".to_string()
        } else {
            format!("{running} agent flows are running. Quitting stops them.")
        };
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("Stop and Quit?")
                .w(crate::style::fixed::DIALOG_WIDTH)
                .child(text.clone())
                .footer(
                    gpui_kit::component::h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            gpui_kit::component::button::Button::new("quit-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            gpui_kit::component::button::Button::new("quit-stop")
                                .label("Stop and Quit")
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                    crate::extensions::host::stop_all(cx);
                                    if can_close(window, cx) {
                                        cx.quit();
                                    }
                                }),
                        ),
                )
        });
        return;
    }
    if can_close(window, cx) {
        cx.quit();
    }
}

/// For closing the window: true when nothing is unsaved. Otherwise opens
/// the unsaved-changes dialog (which quits on its own) and returns false.
pub fn can_close(window: &mut Window, cx: &mut App) -> bool {
    let dirty = current_core(cx).read(cx).dirty_buffers();
    if dirty.is_empty() {
        return true;
    }
    open_quit_dialog(dirty, window, cx);
    false
}

fn open_quit_dialog(dirty: Vec<BufferId>, window: &mut Window, cx: &mut App) {
    let core = current_core(cx);
    let names: Vec<String> = dirty
        .iter()
        .filter_map(|&id| core.read(cx).buffers.get(id).map(|buffer| buffer.name()))
        .collect();
    let question = match names.as_slice() {
        [name] => format!("Save the changes to {name} before quitting?"),
        _ => format!("Save the changes to {} files before quitting?", names.len()),
    };
    let mut list: Vec<String> = names.iter().take(QUIT_LIST_MAX).cloned().collect();
    if names.len() > QUIT_LIST_MAX {
        list.push(format!("and {} more", names.len() - QUIT_LIST_MAX));
    }
    let save_label = if names.len() == 1 { "Save" } else { "Save All" };
    if names.len() == 1 {
        list.clear();
    }
    let save_all: WindowCallback = Rc::new(move |window, cx| {
        save_buffers(dirty.clone(), window, cx, |saved, _, cx| {
            if saved {
                cx.quit();
            }
        })
    });
    window.open_dialog(cx, move |dialog, _, cx| {
        let on_ok = save_all.clone();
        let on_save = save_all.clone();
        let muted = cx.theme().muted_foreground;
        dialog
            .title("Unsaved changes")
            .w(fixed::DIALOG_WIDTH)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
            .child(
                v_flex()
                    .gap(fixed::PANEL_GAP)
                    .child(question.clone())
                    .when(!list.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .text_sm()
                                .children(list.iter().map(|name| div().child(name.clone()))),
                        )
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("Your changes are lost if you don't save them."),
                    ),
            )
            .footer(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("quit-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("quit-discard")
                            .label("Don't Save")
                            .on_click(|_, _, cx| cx.quit()),
                    )
                    .child(
                        Button::new("quit-save")
                            .primary()
                            .label(save_label)
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                on_save(window, cx);
                            }),
                    ),
            )
    });
}
