//! Manage Extensions: the installed and linked extensions with their state
//! and actions (turn on or off, permissions, remove or unlink, reinstall or
//! keep changed files), extensions that did not load, and the catalog with
//! Install / Update. Installing from a folder, zip or git and linking a
//! development folder are in the footer's Install menu (`install.rs`).

use std::rc::Rc;

use carbon_core::extension::{
    Blocked, ExtensionId, Loaded, Origin, describe_capability, installed_hash,
};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, Size, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::install::{self, CatalogView};
use crate::style::fixed;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Installed,
    Catalog,
}

/// Opens the dialog on `tab`.
pub fn open(tab: Tab, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| ManageView::new(tab, window, cx));
    window.open_dialog(cx, move |dialog, window, cx| {
        let locked = super::dirs(cx).is_none();
        let catalog = view.read(cx).catalog.clone();
        let busy = catalog.read(cx).fetching;
        let has_url = install::catalog_config(super::dirs(cx))
            .effective_url()
            .is_some();
        let (source, refresh) = (catalog.clone(), catalog.clone());
        // As big as the Settings dialog; the list takes the height left.
        dialog
            .title("Extensions")
            .w(crate::settings::dialog_size(window).width)
            .child(view.clone())
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Button::new("extensions-install")
                            .label("Install")
                            .dropdown_caret(true)
                            .disabled(locked)
                            .dropdown_menu(|menu, _, _| install::install_menu(menu)),
                    )
                    .child(
                        Button::new("extensions-source")
                            .label("Catalog Source…")
                            .disabled(locked)
                            .on_click(move |_, window, cx| {
                                install::source_dialog(source.clone(), window, cx)
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("extensions-folder")
                            .label("Open Folder")
                            .disabled(locked)
                            .on_click(|_, _, cx| {
                                if let Some(dirs) = super::dirs(cx) {
                                    let dir = dirs.installed_dir().to_path_buf();
                                    _ = std::fs::create_dir_all(&dir);
                                    cx.open_with_system(&dir);
                                }
                            }),
                    )
                    .child(
                        Button::new("extensions-refresh")
                            .label(if busy { "Refreshing…" } else { "Refresh" })
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                super::reload(cx);
                                if has_url {
                                    refresh.update(cx, |view, cx| view.refresh(cx));
                                }
                            }),
                    )
                    .child(
                        Button::new("extensions-close")
                            .primary()
                            .label("Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}

pub struct ManageView {
    tab: Tab,
    search: Entity<InputState>,
    catalog: Entity<CatalogView>,
    _subs: Vec<Subscription>,
}

impl ManageView {
    fn new(tab: Tab, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search extensions"));
        let catalog = cx.new(CatalogView::new);
        let has_url = install::catalog_config(super::dirs(cx))
            .effective_url()
            .is_some();
        if !catalog.read(cx).has_catalog() && has_url {
            catalog.update(cx, |view, cx| view.refresh(cx));
        }
        let _subs = vec![
            cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.observe(&catalog, |_, _, cx| cx.notify()),
            cx.observe_global::<super::Extensions>(|_, cx| cx.notify()),
        ];
        Self {
            tab,
            search,
            catalog,
            _subs,
        }
    }

    fn filter(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_lowercase()
    }

    fn tab_button(&self, tab: Tab, label: String, cx: &mut Context<Self>) -> Button {
        let id = match tab {
            Tab::Installed => "tab-installed",
            Tab::Catalog => "tab-catalog",
        };
        Button::new(id)
            .label(label)
            .with_size(Size::Small)
            .map(|button| {
                if self.tab == tab {
                    button.primary()
                } else {
                    button.ghost()
                }
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.tab = tab;
                cx.notify();
            }))
    }

    fn row(&self, loaded: &Loaded, cx: &mut Context<Self>) -> AnyElement {
        let registry = super::registry(cx);
        let state = super::state(cx).clone();
        let theme = cx.theme();
        let (muted, danger, warning, border) = (
            theme.muted_foreground,
            theme.danger,
            theme.warning,
            theme.border,
        );
        let def = &loaded.def;
        let id = def.id.clone();
        let locked = super::dirs(cx).is_none();
        let installed = loaded.origin == Origin::Installed;
        let record = state.installs.get(&id).cloned();
        let blocked = registry.blocked(&id).cloned();
        let status = match &blocked {
            None => Tag::success().outline().child("On"),
            Some(Blocked::Disabled) => Tag::secondary().outline().child("Off"),
            Some(Blocked::Quarantined(_)) => {
                Tag::danger().outline().child("Turned off after a failure")
            }
            Some(Blocked::Changed(_)) => Tag::danger().outline().child("Files changed"),
            Some(Blocked::Missing(_)) => Tag::warning().outline().child("Needs more"),
        };
        let origin = if installed {
            "Installed"
        } else {
            "Development"
        };
        let trust = record.as_ref().and_then(|record| {
            if record.signed {
                Some("Signed")
            } else {
                record.pinned.then_some("Pinned")
            }
        });
        // Linked folders get every permission they ask for; installed ones what was granted.
        let permissions = (installed && !def.capabilities.is_empty()).then(|| {
            let granted = state.grants.get(&id).cloned().unwrap_or_default();
            let allowed = def
                .capabilities
                .iter()
                .filter(|capability| granted.contains(*capability))
                .count();
            (allowed, def.capabilities.len())
        });
        let mut contributes = Vec::new();
        let count = |n: usize, one: &str, many: &str| match n {
            0 => None,
            1 => Some(format!("1 {one}")),
            n => Some(format!("{n} {many}")),
        };
        contributes.extend(count(def.languages.len(), "language", "languages"));
        contributes.extend(count(
            def.language_servers.len(),
            "language server",
            "language servers",
        ));
        contributes.extend(count(def.themes.len(), "theme file", "theme files"));
        contributes.extend(count(def.panels.len(), "panel", "panels"));
        contributes.extend(count(def.commands.len(), "command", "commands"));
        contributes.extend(count(def.settings.len(), "setting", "settings"));
        if def.wasm.is_some() {
            contributes.push("code".into());
        }
        let action = |label: &'static str, suffix: &str| {
            Button::new(SharedString::from(format!("ext-{suffix}-{id}")))
                .label(label)
                .with_size(Size::Small)
                .disabled(locked)
        };
        let changed = matches!(blocked, Some(Blocked::Changed(_)));
        let toggle = (!changed).then(|| {
            let id = id.clone();
            let turn_on = matches!(blocked, Some(Blocked::Disabled | Blocked::Quarantined(_)));
            action(if turn_on { "Turn On" } else { "Turn Off" }, "toggle")
                .outline()
                .on_click(move |_, window, cx| {
                    let id = id.clone();
                    report(
                        super::update_state(cx, |state| {
                            if turn_on {
                                state.disabled.remove(&id);
                                state.quarantined.remove(&id);
                            } else {
                                state.disabled.insert(id.clone());
                            }
                        }),
                        window,
                        cx,
                    )
                })
        });
        let repair = changed.then(|| {
            let reinstall = record.clone().map(|record| {
                let (id, version) = (id.clone(), def.version.clone());
                action("Reinstall", "reinstall")
                    .outline()
                    .on_click(move |_, window, cx| {
                        install::reinstall(id.clone(), record.clone(), version.clone(), window, cx)
                    })
            });
            let keep = {
                let (id, dir, name) = (id.clone(), loaded.dir.clone(), def.name.clone());
                action("Keep Changes", "keep")
                    .ghost()
                    .on_click(move |_, window, cx| {
                        let (id, dir) = (id.clone(), dir.clone());
                        confirm(
                            window,
                            cx,
                            format!("Keep the changed files of \"{name}\"?"),
                            "Carbon trusts its files as they are now and turns it back on. \
                             It is no longer pinned.",
                            "Keep Changes",
                            move |window, cx| {
                                let hash = installed_hash(&dir);
                                let result = hash.and_then(|hash| {
                                    super::update_state(cx, |state| {
                                        if let Some(record) = state.installs.get_mut(&id) {
                                            record.hash = hash;
                                            record.pinned = false;
                                            record.signed = false;
                                        }
                                        state.changed.remove(&id);
                                    })
                                });
                                report(result, window, cx);
                            },
                        )
                    })
            };
            (reinstall, keep)
        });
        let permissions_button = permissions.map(|_| {
            let id = id.clone();
            action("Permissions", "permissions")
                .ghost()
                .on_click(move |_, window, cx| permissions_dialog(id.clone(), window, cx))
        });
        let remove = {
            let (id, dir, name, origin) = (
                id.clone(),
                loaded.dir.clone(),
                def.name.clone(),
                loaded.origin.clone(),
            );
            let label = if installed { "Remove" } else { "Unlink" };
            action(label, "remove")
                .danger()
                .on_click(move |_, window, cx| {
                    let (id, dir, origin) = (id.clone(), dir.clone(), origin.clone());
                    let (title, message) = if origin == Origin::Installed {
                        (
                            format!("Remove \"{name}\"?"),
                            "Its files are deleted from disk. This cannot be undone.",
                        )
                    } else {
                        (
                            format!("Unlink \"{name}\"?"),
                            "Carbon stops loading it; the folder stays where it is.",
                        )
                    };
                    confirm(window, cx, title, message, label, move |window, cx| {
                        let result = match origin {
                            Origin::Installed => super::dirs(cx)
                                .ok_or_else(|| "safe mode".to_string())
                                .and_then(|dirs| dirs.remove(&origin, &dir))
                                .and_then(|_| super::update_state(cx, |state| state.forget(&id))),
                            Origin::Dev => super::update_state(cx, |state| {
                                state.dev.remove(&dir);
                            }),
                        };
                        report(result, window, cx);
                    })
                })
        };
        let missing = match &blocked {
            Some(Blocked::Missing(missing)) => missing.clone(),
            _ => Vec::new(),
        };
        let reason = match &blocked {
            Some(Blocked::Quarantined(why)) | Some(Blocked::Changed(why)) => Some(why.clone()),
            _ => None,
        };
        let details = match permissions {
            Some((allowed, asked)) => format!(
                "{} · {} · {allowed} of {asked} permissions allowed",
                def.id, def.version
            ),
            None => format!("{} · {}", def.id, def.version),
        };
        h_flex()
            .w_full()
            .gap(fixed::PANEL_GAP)
            .py(fixed::PANEL_GAP)
            .border_b_1()
            .border_color(border)
            .child(install::icon(loaded.icon_file().map(Into::into)))
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
                            .when_some(trust, |row, trust| {
                                row.child(Tag::success().outline().small().child(trust))
                            }),
                    )
                    .child(div().text_xs().text_color(muted).child(details))
                    .when(!def.description.is_empty(), |column| {
                        column.child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(def.description.clone()),
                        )
                    })
                    .when(!contributes.is_empty(), |column| {
                        column.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("Adds {}", contributes.join(", "))),
                        )
                    })
                    .when_some(record, |column, record| {
                        let from = match (&record.commit, &record.path) {
                            (Some(commit), Some(path)) => format!(
                                "{} /{path} @ {}",
                                record.source,
                                commit.chars().take(10).collect::<String>()
                            ),
                            (Some(commit), None) => format!(
                                "{} @ {}",
                                record.source,
                                commit.chars().take(10).collect::<String>()
                            ),
                            _ => record.source.clone(),
                        };
                        column.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                // Keep the end (revision, commit, hash) visible.
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis_start()
                                .child(format!(
                                    "From {from} · {}",
                                    install::short_hash(&record.hash)
                                )),
                        )
                    })
                    .when(!installed, |column| {
                        column.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis_start()
                                .child(format!("Linked from {}", loaded.dir.display())),
                        )
                    })
                    .children(missing.iter().map(|missing| {
                        let offer = install::catalog(cx)
                            .filter(|_| {
                                missing.kind == carbon_core::extension::RequireKind::Extension
                            })
                            .and_then(|catalog| {
                                let range = semver::VersionReq::parse(&missing.range).ok()?;
                                let wanted = ExtensionId::new(missing.id.clone());
                                let version =
                                    catalog.get(&wanted)?.best(Some(&range))?.version.clone();
                                Some(
                                    Button::new(SharedString::from(format!(
                                        "ext-needs-{id}-{}",
                                        missing.id
                                    )))
                                    .label(format!("Install {version}"))
                                    .xsmall()
                                    .outline()
                                    .disabled(locked)
                                    .on_click(
                                        move |_, window, cx| {
                                            install::install_listed(
                                                &wanted,
                                                Some(&range),
                                                window,
                                                cx,
                                            )
                                        },
                                    ),
                                )
                            });
                        h_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(warning)
                                    .child(missing.to_string()),
                            )
                            .children(offer)
                    }))
                    .when_some(reason, |column, reason| {
                        column.child(div().text_xs().text_color(danger).child(reason))
                    }),
            )
            .child(
                h_flex()
                    .gap_1()
                    .children(toggle)
                    .when_some(repair, |buttons, (reinstall, keep)| {
                        buttons.children(reinstall).child(keep)
                    })
                    .children(permissions_button)
                    .child(remove),
            )
            .into_any_element()
    }
}

/// Shows a failure of a state change as a notification.
fn report(result: Result<(), String>, window: &mut Window, cx: &mut App) {
    if let Err(err) = result {
        window.push_notification(format!("Cannot change extensions: {err}"), cx);
    }
}

impl Render for ManageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list_height = crate::settings::dialog_size(window).height - fixed::MANAGE_CHROME_HEIGHT;
        let registry = super::registry(cx);
        let filter = self.filter(cx);
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let installed_count = registry.all().len();
        let tabs = h_flex()
            .gap_1()
            .child(self.tab_button(Tab::Catalog, "Catalog".into(), cx))
            .child(self.tab_button(Tab::Installed, format!("Installed ({installed_count})"), cx))
            .child(div().flex_1())
            .child(
                div()
                    .w(fixed::SETTINGS_TEXT_WIDTH)
                    .child(Input::new(&self.search).small()),
            );
        let body: Vec<AnyElement> =
            match self.tab {
                Tab::Installed => {
                    let mut rows: Vec<AnyElement> = registry
                        .all()
                        .iter()
                        .filter(|loaded| {
                            filter.is_empty()
                                || format!(
                                    "{} {} {}",
                                    loaded.def.name, loaded.def.id, loaded.def.description
                                )
                                .to_lowercase()
                                .contains(&filter)
                        })
                        .map(|loaded| self.row(loaded, cx))
                        .collect();
                    if registry.all().is_empty() {
                        rows.push(
                            div()
                                .py(fixed::PANEL_GAP)
                                .text_sm()
                                .text_color(muted)
                                .child(
                                    "No extensions yet. Carbon has no languages, themes or tools \
                                 built in beyond its own: add them from the Catalog, or with \
                                 Install.",
                                )
                                .into_any_element(),
                        );
                    }
                    let rejected = registry.rejected().to_vec();
                    if !rejected.is_empty() {
                        rows.push(
                            div()
                                .pt(fixed::PANEL_PADDING)
                                .font_weight(FontWeight::MEDIUM)
                                .child("Did not load")
                                .into_any_element(),
                        );
                        rows.extend(rejected.into_iter().map(|rejected| {
                            v_flex()
                                .py(fixed::PANEL_GAP)
                                .gap_1()
                                .child(div().text_xs().text_color(muted).child(rejected.source))
                                .children(rejected.problems.into_iter().map(|problem| {
                                    div().text_xs().text_color(danger).child(problem)
                                }))
                                .into_any_element()
                        }));
                    }
                    rows
                }
                Tab::Catalog => self.catalog.read(cx).body(&filter, cx),
            };
        v_flex()
            .gap(fixed::PANEL_GAP)
            .child(tabs)
            .when(super::dirs(cx).is_none(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("Extensions cannot be changed: Carbon has no config folder."),
                )
            })
            .child(
                v_flex()
                    .id("manage-extensions")
                    .h(list_height)
                    .overflow_y_scroll()
                    .children(body),
            )
    }
}

/// The permissions an installed extension asks for, with Allow / Revoke.
fn permissions_dialog(id: ExtensionId, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let registry = super::registry(cx);
        let asked = registry
            .get(&id)
            .map(|loaded| loaded.def.capabilities.clone())
            .unwrap_or_default();
        let granted = super::state(cx)
            .grants
            .get(&id)
            .cloned()
            .unwrap_or_default();
        let all = asked.iter().all(|capability| granted.contains(capability));
        let muted = cx.theme().muted_foreground;
        let (allow_id, allow_asked, revoke_id) = (id.clone(), asked.clone(), id.clone());
        dialog
            .title("Permissions")
            .w(fixed::DIALOG_WIDTH)
            .child(
                v_flex()
                    .gap_1()
                    .children(asked.iter().map(|capability| {
                        let on = granted.contains(capability);
                        h_flex()
                            .gap_1()
                            .text_sm()
                            .child(describe_capability(capability))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("({capability})")),
                            )
                            .child(if on {
                                Tag::success().outline().small().child("Allowed")
                            } else {
                                Tag::secondary().outline().small().child("Not allowed")
                            })
                    }))
                    .child(div().pt(fixed::PANEL_GAP).text_xs().text_color(muted).child(
                        "Without a permission, the extension's requests for it fail; it keeps \
                         running otherwise.",
                    )),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("permissions-revoke")
                            .danger()
                            .label("Revoke All")
                            .disabled(granted.is_empty())
                            .on_click(move |_, window, cx| {
                                let id = revoke_id.clone();
                                report(
                                    super::update_state(cx, |state| {
                                        state.grants.remove(&id);
                                    }),
                                    window,
                                    cx,
                                );
                                window.refresh();
                            }),
                    )
                    .child(
                        Button::new("permissions-allow")
                            .label("Allow All")
                            .disabled(all)
                            .on_click(move |_, window, cx| {
                                let (id, asked) = (allow_id.clone(), allow_asked.clone());
                                report(
                                    super::update_state(cx, |state| {
                                        state.grants.insert(id, asked.into_iter().collect());
                                    }),
                                    window,
                                    cx,
                                );
                                window.refresh();
                            }),
                    )
                    .child(
                        Button::new("permissions-close")
                            .primary()
                            .label("Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}

/// A second dialog asking to confirm a step.
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
