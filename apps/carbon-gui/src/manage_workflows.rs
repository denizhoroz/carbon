//! Manage Workflows dialog (`project/built/workflows.md`): every workflow with
//! where it comes from and its state, files that did not load, and the
//! actions: turn on/off (also lifts a quarantine), duplicate, reset, remove,
//! open the folder, reload. The rules live in the core registry (the Editor
//! cannot be turned off or removed; Carbon's own packages cannot be removed).

use std::rc::Rc;

use carbon_core::workflow::{Origin, WorkflowDef};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, Size, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::style::fixed;
use crate::workbench::{CurrentWorkbench, Workbench};
use crate::workflows;

/// Opens the dialog.
pub fn open(window: &mut Window, cx: &mut App) {
    let view = cx.new(WorkflowsView::new);
    window.open_dialog(cx, move |dialog, _, _| {
        let reload = view.clone();
        dialog
            .title("Manage Workflows")
            .w(fixed::MANAGE_WIDTH)
            .child(view.clone())
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("workflows-folder")
                            .label("Open Folder")
                            .disabled(workflows::dirs().is_none())
                            .on_click(|_, _, cx| open_folder(cx)),
                    )
                    .child(Button::new("workflows-reload").label("Reload").on_click(
                        move |_, window, cx| {
                            reload.update(cx, |view, cx| {
                                view.with_workbench(window, cx, |w, window, cx| {
                                    w.reload_workflows(window, cx)
                                })
                            })
                        },
                    ))
                    .child(
                        Button::new("workflows-close")
                            .primary()
                            .label("Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}

/// Opens the preferences folder (creating `user/` so there is a place to
/// put a workflow).
fn open_folder(cx: &mut App) {
    let Some(dirs) = workflows::dirs() else {
        return;
    };
    let user = dirs.user_dir();
    if let Err(err) = std::fs::create_dir_all(&user) {
        eprintln!("cannot create {}: {err}", user.display());
    }
    cx.open_with_system(dirs.root());
}

pub struct WorkflowsView {
    workbench: Option<Entity<Workbench>>,
    _sub: Option<Subscription>,
}

impl WorkflowsView {
    fn new(cx: &mut Context<Self>) -> Self {
        let workbench = cx
            .try_global::<CurrentWorkbench>()
            .and_then(|current| current.0.upgrade());
        let _sub = workbench
            .as_ref()
            .map(|workbench| cx.observe(workbench, |_, _, cx| cx.notify()));
        Self { workbench, _sub }
    }

    /// Runs `f` on the workbench, then redraws.
    fn with_workbench(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Workbench, &mut Window, &mut Context<Workbench>),
    ) {
        if let Some(workbench) = &self.workbench {
            workbench.update(cx, |workbench, cx| f(workbench, window, cx));
        }
        cx.notify();
    }

    fn row(&self, def: &WorkflowDef, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = workflows::registry();
        let theme = cx.theme();
        let (muted, danger, border) = (theme.muted_foreground, theme.danger, theme.border);
        let locked = workflows::dirs().is_none();
        let id = def.id.clone();
        let (off, quarantine, saved_layout) = self
            .workbench
            .as_ref()
            .map(|workbench| {
                let workbench = workbench.read(cx);
                let (off, quarantine) = workbench.workflow_flags(&id);
                (off, quarantine, workbench.has_saved_layout(&id))
            })
            .unwrap_or_default();
        let overlay = registry.overlay(&id).is_some();
        let origin = match registry.origin(&id) {
            Some(Origin::Installed(_)) => "Installed",
            Some(Origin::User(_)) => "User",
            _ => "Carbon",
        };
        let status = if quarantine.is_some() {
            Tag::danger().outline().child("Turned off after a crash")
        } else if off {
            Tag::secondary().outline().child("Off")
        } else if def.hidden {
            Tag::secondary().outline().child("Hidden")
        } else {
            Tag::success().outline().child("On")
        };
        let entity = cx.entity();
        let action = |label: &'static str, suffix: &str| {
            Button::new(SharedString::from(format!("workflow-{suffix}-{id}")))
                .label(label)
                .with_size(Size::Small)
                .disabled(locked)
        };

        let toggle = registry.can_turn_off(&id).then(|| {
            let (entity, id) = (entity.clone(), id.clone());
            let turn_on = off || quarantine.is_some();
            action(if turn_on { "Turn On" } else { "Turn Off" }, "toggle")
                .outline()
                .on_click(move |_, window, cx| {
                    entity.update(cx, |view, cx| {
                        view.with_workbench(window, cx, |w, window, cx| {
                            w.set_workflow_on(&id, turn_on, window, cx)
                        })
                    })
                })
        });
        let duplicate = {
            let (entity, id) = (entity.clone(), id.clone());
            action("Duplicate", "duplicate")
                .ghost()
                .on_click(move |_, window, cx| {
                    entity.update(cx, |view, cx| {
                        view.with_workbench(window, cx, |w, window, cx| {
                            w.duplicate_workflow(&id, window, cx);
                        })
                    })
                })
        };
        let reset = (overlay || saved_layout).then(|| {
            let (entity, id, name) = (entity.clone(), id.clone(), def.name.clone());
            action("Reset", "reset")
                .ghost()
                .on_click(move |_, window, cx| {
                    let (entity, id) = (entity.clone(), id.clone());
                    confirm(
                        window,
                        cx,
                        format!("Reset \"{name}\"?"),
                        "Its saved layout and any changes from an overlay file are deleted. \
                         Named layouts stay.",
                        "Reset",
                        move |window, cx| {
                            entity.update(cx, |view, cx| {
                                view.with_workbench(window, cx, |w, window, cx| {
                                    w.reset_workflow(&id, window, cx)
                                })
                            })
                        },
                    )
                })
        });
        let remove = registry.removable(&id).then(|| {
            let (entity, id, name) = (entity.clone(), id.clone(), def.name.clone());
            action("Remove", "remove")
                .danger()
                .on_click(move |_, window, cx| {
                    let (entity, id) = (entity.clone(), id.clone());
                    confirm(
                        window,
                        cx,
                        format!("Remove \"{name}\"?"),
                        "Its folder is deleted from disk. This cannot be undone.",
                        "Remove",
                        move |window, cx| {
                            entity.update(cx, |view, cx| {
                                view.with_workbench(window, cx, |w, window, cx| {
                                    w.remove_workflow(&id, window, cx)
                                })
                            })
                        },
                    )
                })
        });

        h_flex()
            .w_full()
            .gap(fixed::PANEL_GAP)
            .py(fixed::PANEL_GAP)
            .border_b_1()
            .border_color(border)
            .child(workflows::icon(def).with_size(Size::Size(fixed::SECTION_ICON_SIZE)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(def.name.clone()),
                            )
                            .child(Tag::secondary().outline().small().child(origin))
                            .child(status.small())
                            .when(overlay, |row| {
                                row.child(Tag::info().outline().small().child("Changed"))
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{} · {}", def.id, def.version)),
                    )
                    .when_some(quarantine, |row, reason| {
                        row.child(div().text_xs().text_color(danger).child(reason))
                    }),
            )
            .child(
                h_flex()
                    .gap_1()
                    .children(toggle)
                    .child(duplicate)
                    .children(reset)
                    .children(remove),
            )
    }
}

impl Render for WorkflowsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = workflows::registry();
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let rows: Vec<AnyElement> = registry
            .workflows()
            .iter()
            .map(|def| self.row(def, cx).into_any_element())
            .collect();
        let rejected = registry.rejected().to_vec();
        v_flex()
            .id("manage-workflows")
            .h(fixed::MANAGE_BODY_HEIGHT)
            .overflow_y_scroll()
            .when(workflows::dirs().is_none(), |list| {
                list.child(div().text_sm().text_color(muted).pb(fixed::PANEL_GAP).child(
                    "Safe mode: workflows cannot be changed. Restart Carbon to leave safe mode.",
                ))
            })
            .children(rows)
            .when(!rejected.is_empty(), |list| {
                list.child(
                    div()
                        .pt(fixed::PANEL_PADDING)
                        .font_weight(FontWeight::MEDIUM)
                        .child("Did not load"),
                )
                .children(rejected.into_iter().map(|rejected| {
                    v_flex()
                        .py(fixed::PANEL_GAP)
                        .gap_1()
                        .child(div().text_xs().text_color(muted).child(rejected.source))
                        .children(
                            rejected
                                .problems
                                .into_iter()
                                .map(|problem| div().text_xs().text_color(danger).child(problem)),
                        )
                }))
            })
    }
}

/// A second dialog asking to confirm a step that deletes something.
fn confirm(
    window: &mut Window,
    cx: &mut App,
    title: String,
    message: &'static str,
    action: &'static str,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
) {
    let on_confirm = Rc::new(on_confirm);
    window.open_dialog(cx, move |dialog, _, _| {
        let on_confirm = on_confirm.clone();
        dialog
            .title(title.clone())
            .w(fixed::DIALOG_WIDTH)
            .child(div().text_sm().child(message))
            .footer(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("confirm-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(Button::new("confirm-ok").danger().label(action).on_click(
                        move |_, window, cx| {
                            window.close_dialog(cx);
                            on_confirm(window, cx);
                        },
                    )),
            )
    });
}
