//! Installing workflow packages from Manage Workflows (`project/built/workflows.md`):
//! a folder, a zip, a git repository at a tag, branch or commit, or the
//! catalog (`carbon_core::workflow::Catalog`). Reading and fetching run off
//! the UI thread; fetching shows a dialog with Cancel. Then the install
//! dialog says what installing does (new, update, downgrade; permissions;
//! requirements; pins) and installs on the user's click.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use carbon_core::workflow::{
    CATALOG_ENV, CATALOG_SIGNATURE, Catalog, CatalogConfig, CatalogFiles, CatalogPackage,
    FetchControl, InstallKind, InstallRecord, InstallSource, Origin, Package, Pin, Signing,
    WorkflowId, describe_capability, env_url, fetch_catalog, fetch_from_catalog, fetch_git,
    plan_install, read_package,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable, Size, WindowExt, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use semver::VersionReq;

use crate::style::fixed;
use crate::workbench::{CurrentWorkbench, WindowCallback};
use crate::workflows;

/// The footer's Install menu.
pub fn install_menu(menu: PopupMenu) -> PopupMenu {
    menu.item(
        PopupMenuItem::new("From Folder…")
            .on_click(|_, window, cx| pick_package(false, window, cx)),
    )
    .item(PopupMenuItem::new("From Zip…").on_click(|_, window, cx| pick_package(true, window, cx)))
    .item(PopupMenuItem::new("From Git…").on_click(|_, window, cx| git_dialog(window, cx)))
}

/// Asks for a package folder or `.zip`, reads and checks it off the UI
/// thread, then shows what installing it would do.
fn pick_package(zip: bool, window: &mut Window, cx: &mut App) {
    let picked = cx.prompt_for_paths(PathPromptOptions {
        files: zip,
        directories: !zip,
        multiple: false,
        prompt: Some(if zip { "Install Zip" } else { "Install Folder" }.into()),
    });
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let source = InstallSource::from_path(&path);
            let read = if zip && !matches!(source, InstallSource::Zip(_)) {
                Err(vec![format!("{} is not a .zip file", path.display())])
            } else {
                cx.background_executor()
                    .spawn(async move { read_package(source) })
                    .await
            };
            _ = cx.update(|window, cx| match read {
                Ok(package) => install_dialog(package, window, cx),
                Err(problems) => problems_dialog(path.display().to_string(), problems, window, cx),
            });
        })
        .detach();
}

/// Asks for a repository URL and a tag, branch or commit id.
fn git_dialog(window: &mut Window, cx: &mut App) {
    let url = cx
        .new(|cx| InputState::new(window, cx).placeholder("https://example.com/acme/workflow.git"));
    let rev = cx.new(|cx| {
        InputState::new(window, cx).placeholder("Tag, branch or commit id (empty: default branch)")
    });
    let submit: WindowCallback = {
        let (url, rev) = (url.clone(), rev.clone());
        Rc::new(move |window, cx| {
            let url = url.read(cx).value().trim().to_string();
            let rev = rev.read(cx).value().trim().to_string();
            if url.is_empty() {
                return;
            }
            window.close_dialog(cx);
            let what = if rev.is_empty() {
                url.clone()
            } else {
                format!("{url} @ {rev}")
            };
            fetch_then_install(
                what,
                move |control| fetch_git(&url, &rev, control),
                window,
                cx,
            );
        })
    };
    for input in [&url, &rev] {
        let on_enter = submit.clone();
        window
            .subscribe(input, cx, move |_, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    on_enter(window, cx);
                }
            })
            .detach();
    }
    let (dialog_url, dialog_rev) = (url.clone(), rev.clone());
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        let label = |text: &'static str| div().text_sm().child(text);
        let on_fetch = submit.clone();
        dialog
            .title("Install from Git")
            .w(fixed::DIALOG_WIDTH)
            .child(
                v_flex()
                    .gap(fixed::PANEL_GAP)
                    .child(label("Repository"))
                    .child(Input::new(&dialog_url))
                    .child(label("Revision"))
                    .child(Input::new(&dialog_rev))
                    .child(div().text_xs().text_color(muted).child(
                        "A full commit id pins the install; a tag or branch can move later.",
                    )),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("git-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("git-fetch")
                            .primary()
                            .label("Fetch")
                            .on_click(move |_, window, cx| on_fetch(window, cx)),
                    ),
            )
    });
    url.update(cx, |input, cx| input.focus(window, cx));
}

/// Runs `job` (a fetch) off the UI thread behind a dialog showing its
/// progress, with Cancel; then shows the install dialog or what went wrong.
/// The dialog closes only through Cancel, so closing it here never closes
/// another one.
fn fetch_then_install(
    what: String,
    job: impl FnOnce(&FetchControl) -> Result<Package, Vec<String>> + Send + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let control = Arc::new(FetchControl::default());
    let dialog_control = control.clone();
    let text = format!("Fetching {what}…");
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        let on_cancel = dialog_control.clone();
        dialog
            .title("Fetching")
            .w(fixed::DIALOG_WIDTH)
            .keyboard(false)
            .overlay_closable(false)
            .close_button(false)
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_sm().text_color(muted).child(text.clone()))
                    .when_some(dialog_control.status(), |body, status| {
                        body.child(div().text_xs().text_color(muted).child(status))
                    }),
            )
            .footer(
                h_flex().w_full().justify_end().child(
                    Button::new("fetch-cancel")
                        .label("Cancel")
                        .on_click(move |_, window, cx| {
                            on_cancel.cancel();
                            window.close_dialog(cx);
                        }),
                ),
            )
    });
    let done = Arc::new(AtomicBool::new(false));
    let (job_control, job_done) = (control.clone(), done.clone());
    let task = cx.background_executor().spawn(async move {
        let result = job(&job_control);
        job_done.store(true, Ordering::Relaxed);
        result
    });
    window
        .spawn(cx, async move |cx| {
            // Redraw the dialog while the job runs: its byte count moves.
            while !done.load(Ordering::Relaxed) && !control.is_cancelled() {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                _ = cx.update(|window, _| window.refresh());
            }
            let result = task.await;
            if control.is_cancelled() {
                return;
            }
            _ = cx.update(|window, cx| {
                window.close_dialog(cx);
                match result {
                    Ok(package) => install_dialog(package, window, cx),
                    Err(problems) => problems_dialog(what, problems, window, cx),
                }
            });
        })
        .detach();
}

/// Why a package cannot be installed.
fn problems_dialog(source: String, problems: Vec<String>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let (muted, danger) = (cx.theme().muted_foreground, cx.theme().danger);
        dialog
            .title("Cannot Install")
            .w(fixed::DIALOG_WIDTH)
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_xs().text_color(muted).child(source.clone()))
                    .children(
                        problems.iter().map(|problem| {
                            div().text_sm().text_color(danger).child(problem.clone())
                        }),
                    ),
            )
            .footer(
                h_flex().w_full().justify_end().child(
                    Button::new("problems-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    });
}

/// What installing `package` does: new, update, reinstall or downgrade; the
/// permissions it asks for; requirements; pins; clashes. Install grants the
/// permissions.
fn install_dialog(package: Package, window: &mut Window, cx: &mut App) {
    let Some(workbench) = cx
        .try_global::<CurrentWorkbench>()
        .and_then(|current| current.0.upgrade())
    else {
        return;
    };
    let granted = workbench.read(cx).grants(&package.def.id);
    let plan = plan_install(&package, &workflows::registry(), &granted);
    let package = Rc::new(package);
    let plan = Rc::new(plan);
    window.open_dialog(cx, move |dialog, _, cx| {
        let theme = cx.theme();
        let (muted, danger, warning) = (theme.muted_foreground, theme.danger, theme.warning);
        let def = &package.def;
        let (title, change, risky) = match &plan.kind {
            InstallKind::New => ("Install Workflow", "A new workflow.".to_string(), false),
            InstallKind::Update { from } => (
                "Update Workflow",
                format!("Updates {from} to {}.", def.version),
                false,
            ),
            InstallKind::Reinstall => (
                "Reinstall Workflow",
                format!("{} is installed; its files are replaced.", def.version),
                false,
            ),
            InstallKind::Downgrade { from } => (
                "Install Older Version",
                format!("{from} is installed; this goes back to {}.", def.version),
                true,
            ),
        };
        let record = package.record();
        let pin = if record.signed {
            "Pinned by a signed catalog: commit and content hash checked."
        } else if package.pin.is_some() {
            "Pinned: commit and content hash checked."
        } else if record.pinned {
            "Pinned to a commit."
        } else {
            "Not pinned: the same source may hold other files later."
        };
        let asks = !plan.new_capabilities.is_empty();
        let section = |label: &'static str| {
            div()
                .pt(fixed::PANEL_GAP)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(label)
        };
        let capability = |text: &String, new: bool| {
            h_flex()
                .gap_1()
                .text_sm()
                .child(describe_capability(text))
                .child(div().text_xs().text_color(muted).child(format!("({text})")))
                .when(new, |row| {
                    row.child(Tag::warning().outline().small().child("New"))
                })
        };
        let (accept_plan, accept_package, accept_workbench) =
            (plan.clone(), package.clone(), workbench.clone());
        dialog
            .title(title)
            .w(fixed::DIALOG_WIDTH)
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                workflows::icon(def)
                                    .with_size(Size::Size(fixed::SECTION_ICON_SIZE)),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(def.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("{} · {}", def.id, def.version)),
                            ),
                    )
                    .when(!def.description.is_empty(), |body| {
                        body.child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(def.description.clone()),
                        )
                    })
                    .child(
                        div()
                            .text_sm()
                            .when(risky, |text| text.text_color(warning))
                            .child(change),
                    )
                    .child(div().text_xs().text_color(muted).child(format!(
                        "{} files, {} KB, from {}",
                        package.files.len(),
                        package.bytes.div_ceil(1024),
                        package.source.describe()
                    )))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{pin} {}", short_hash(&package.hash))),
                    )
                    .child(section("Permissions"))
                    .map(|body| {
                        if def.capabilities.is_empty() {
                            body.child(
                                div()
                                    .text_sm()
                                    .text_color(muted)
                                    .child("Needs no permissions."),
                            )
                        } else {
                            body.children(plan.new_capabilities.iter().map(|c| capability(c, true)))
                                .children(
                                    plan.kept_capabilities.iter().map(|c| capability(c, false)),
                                )
                        }
                    })
                    .when(!plan.dropped_capabilities.is_empty(), |body| {
                        body.child(div().text_xs().text_color(muted).child(format!(
                            "No longer asked for: {}",
                            plan.dropped_capabilities.join(", ")
                        )))
                    })
                    .when(!plan.needs.is_empty(), |body| {
                        body.child(section("Needs"))
                            .children(plan.needs.iter().map(|missing| {
                                div()
                                    .text_sm()
                                    .text_color(warning)
                                    .child(missing.to_string())
                            }))
                            .child(div().text_xs().text_color(muted).child(
                                "It installs, but stays off until these are installed \
                                 (Manage Workflows offers them when the catalog has them).",
                            ))
                    })
                    .when(plan.layout_changed, |body| {
                        body.child(div().text_sm().text_color(muted).child(
                            "Its default layout changed; your saved layout for it is replaced.",
                        ))
                    })
                    .when(!plan.problems.is_empty(), |body| {
                        body.child(section("Cannot install"))
                            .children(plan.problems.iter().map(|problem| {
                                div().text_sm().text_color(danger).child(problem.clone())
                            }))
                    }),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("install-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("install-ok")
                            .primary()
                            .label(if asks { "Allow and Install" } else { "Install" })
                            .disabled(!plan.problems.is_empty())
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                accept_workbench.update(cx, |workbench, cx| {
                                    workbench.install_package(
                                        &accept_package,
                                        &accept_plan,
                                        window,
                                        cx,
                                    )
                                });
                            }),
                    ),
            )
    });
}

/// `sha256:` and the first 12 hex digits.
pub fn short_hash(hash: &str) -> String {
    hash.chars().take("sha256:".len() + 12).collect()
}

// ---- catalog -----------------------------------------------------------------

/// The last catalog loaded (signature checked), from `state/catalog.json`
/// until a refresh.
static CATALOG: RwLock<Option<Arc<Catalog>>> = RwLock::new(None);

/// `catalog.toml` (repository URL, trusted keys); defaults when missing or
/// unreadable.
pub fn catalog_config() -> CatalogConfig {
    workflows::dirs()
        .and_then(|dirs| CatalogConfig::load(&dirs.catalog_config_file()).ok())
        .unwrap_or_default()
}

/// The cached catalog's two files.
fn cache_paths() -> Option<(PathBuf, PathBuf)> {
    let text = workflows::dirs()?.catalog_file();
    let signature = text.with_file_name(CATALOG_SIGNATURE);
    Some((text, signature))
}

/// The catalog as last fetched: read from its cache the first time, with
/// the signature checked against today's trusted keys.
pub fn catalog() -> Option<Arc<Catalog>> {
    if let Some(catalog) = CATALOG.read().ok().and_then(|slot| slot.clone()) {
        return Some(catalog);
    }
    let (text, signature) = cache_paths()?;
    let files = CatalogFiles {
        text: std::fs::read_to_string(text).ok()?,
        signature: std::fs::read_to_string(signature).ok(),
    };
    let (catalog, _) = Catalog::load(&files, &catalog_config().keys).ok()?;
    let catalog = Arc::new(catalog);
    if let Ok(mut slot) = CATALOG.write() {
        *slot = Some(catalog.clone());
    }
    Some(catalog)
}

/// Keeps `files` as the cached catalog and `catalog` as the loaded one.
fn remember(files: &CatalogFiles, catalog: Arc<Catalog>) {
    if let Some((text, signature)) = cache_paths() {
        let saved = text
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&text, &files.text))
            .and_then(|_| match &files.signature {
                Some(sig) => std::fs::write(&signature, sig),
                None => match std::fs::remove_file(&signature) {
                    Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
                    _ => Ok(()),
                },
            });
        if let Err(err) = saved {
            eprintln!("cannot write {}: {err}", text.display());
        }
    }
    if let Ok(mut slot) = CATALOG.write() {
        *slot = Some(catalog);
    }
}

/// Drops the loaded and cached catalog (its source or keys changed).
fn forget_catalog() {
    if let Ok(mut slot) = CATALOG.write() {
        *slot = None;
    }
    if let Some((text, signature)) = cache_paths() {
        let _ = std::fs::remove_file(text);
        let _ = std::fs::remove_file(signature);
    }
}

/// Installs the catalog's best version of `id` within `range` (a missing
/// requirement's Install button).
pub fn install_listed(
    id: &WorkflowId,
    range: Option<&VersionReq>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(catalog) = catalog() else {
        return;
    };
    let Some(listed) = catalog.get(id).cloned() else {
        return;
    };
    install_from_catalog(listed, range, catalog.is_signed(), window, cx);
}

fn install_from_catalog(
    listed: CatalogPackage,
    range: Option<&VersionReq>,
    signed: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(version) = listed.best(range).cloned() else {
        return;
    };
    let what = format!("{} {}", listed.name, version.version);
    fetch_then_install(
        what,
        move |control| fetch_from_catalog(&listed, &version, signed, control),
        window,
        cx,
    );
}

/// Fetches an installed package again from where it came from, so its files
/// are the ones recorded at install (Manage Workflows > Reinstall after its
/// files changed). A pinned install keeps its pin; for an unpinned one the
/// source must still hold the same files.
pub fn reinstall(
    id: WorkflowId,
    record: InstallRecord,
    version: String,
    window: &mut Window,
    cx: &mut App,
) {
    let pin = record.pinned.then(|| Pin {
        id,
        version,
        hash: record.hash.clone(),
        signed: record.signed,
    });
    let what = match &record.commit {
        Some(commit) => format!("{} @ {}", record.source, &commit[..commit.len().min(10)]),
        None => record.source.clone(),
    };
    fetch_then_install(
        what,
        move |control| {
            let mut package = match &record.commit {
                Some(commit) => fetch_git(&record.source, commit, control)?,
                None => read_package(InstallSource::from_path(&record.source))?,
            };
            if pin.is_none() && package.hash != record.hash {
                return Err(vec![format!(
                    "{} no longer holds the files that were installed (the content hash \
                     differs). Install it again from its source to take the new files.",
                    record.source
                )]);
            }
            package.pin = pin;
            Ok(package)
        },
        window,
        cx,
    );
}

/// The catalog's packages with Install / Update; Refresh fetches it again,
/// Source… sets the repository and the trusted keys.
pub fn catalog_dialog(window: &mut Window, cx: &mut App) {
    let view = cx.new(CatalogView::new);
    if view.read(cx).catalog.is_none() {
        view.update(cx, |view, cx| view.refresh(cx));
    }
    window.open_dialog(cx, move |dialog, _, cx| {
        let (refresh, source) = (view.clone(), view.clone());
        let busy = view.read(cx).fetching;
        let locked = workflows::dirs().is_none();
        dialog
            .title("Workflow Catalog")
            .w(fixed::MANAGE_WIDTH)
            .child(view.clone())
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Button::new("catalog-source")
                            .label("Source…")
                            .disabled(locked)
                            .on_click(move |_, window, cx| {
                                source_dialog(source.clone(), window, cx)
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("catalog-refresh")
                            .label(if busy { "Refreshing…" } else { "Refresh" })
                            .disabled(busy || catalog_config().effective_url().is_none())
                            .on_click(move |_, _, cx| {
                                refresh.update(cx, |view, cx| view.refresh(cx))
                            }),
                    )
                    .child(
                        Button::new("catalog-close")
                            .primary()
                            .label("Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}

/// Asks for the catalog repository and the keys trusted to sign it; saves
/// them to `catalog.toml` and fetches the catalog again.
fn source_dialog(view: Entity<CatalogView>, window: &mut Window, cx: &mut App) {
    let config = catalog_config();
    let url = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("https://example.com/acme/catalog.git")
            .default_value(config.url.clone().unwrap_or_default())
    });
    let keys = cx.new(|cx| {
        TextareaState::new(window, cx)
            .rows(4)
            .placeholder(
                "One minisign public key per line (RWQ…); empty: signatures are not checked",
            )
            .default_value(config.keys.join("\n"))
    });
    let error: Rc<RefCell<Option<String>>> = Rc::default();
    let (dialog_url, dialog_keys, dialog_error) = (url.clone(), keys.clone(), error.clone());
    window.open_dialog(cx, move |dialog, _, cx| {
        let (muted, danger) = (cx.theme().muted_foreground, cx.theme().danger);
        let label = |text: &'static str| div().text_sm().child(text);
        let (save_url, save_keys, save_error, save_view) = (
            dialog_url.clone(),
            dialog_keys.clone(),
            dialog_error.clone(),
            view.clone(),
        );
        dialog
            .title("Catalog Source")
            .w(fixed::DIALOG_WIDTH)
            .child(
                v_flex()
                    .gap(fixed::PANEL_GAP)
                    .child(label("Repository"))
                    .child(Input::new(&dialog_url))
                    .when_some(env_url(), |body, url| {
                        body.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("{CATALOG_ENV} is set, so {url} is used instead.")),
                        )
                    })
                    .child(label("Trusted keys"))
                    .child(Textarea::new(&dialog_keys))
                    .child(div().text_xs().text_color(muted).child(
                        "With keys, the catalog must carry catalog.json.minisig made by one of \
                         them (minisign -Sm catalog.json); without, it is used unchecked.",
                    ))
                    .when_some(dialog_error.borrow().clone(), |body, err| {
                        body.child(div().text_sm().text_color(danger).child(err))
                    }),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("source-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(Button::new("source-save").primary().label("Save").on_click(
                        move |_, window, cx| {
                            let url = save_url.read(cx).value().trim().to_string();
                            let keys = save_keys
                                .read(cx)
                                .value()
                                .lines()
                                .map(str::trim)
                                .filter(|key| !key.is_empty())
                                .map(str::to_string)
                                .collect();
                            let config = CatalogConfig {
                                url: (!url.is_empty()).then_some(url),
                                keys,
                            };
                            let saved = match config.check().into_iter().next() {
                                Some(problem) => Err(problem),
                                None => workflows::dirs()
                                    .ok_or_else(|| "safe mode".to_string())
                                    .and_then(|dirs| config.save(&dirs.catalog_config_file())),
                            };
                            match saved {
                                Ok(()) => {
                                    window.close_dialog(cx);
                                    forget_catalog();
                                    save_view.update(cx, |view, cx| {
                                        view.catalog = None;
                                        view.problems.clear();
                                        view.refresh(cx);
                                    });
                                }
                                Err(err) => {
                                    *save_error.borrow_mut() = Some(err);
                                    window.refresh();
                                }
                            }
                        },
                    )),
            )
    });
    url.update(cx, |input, cx| input.focus(window, cx));
}

struct CatalogView {
    catalog: Option<Arc<Catalog>>,
    /// Entries left out, or why the last refresh failed.
    problems: Vec<String>,
    fetching: bool,
    _task: Option<Task<()>>,
}

impl CatalogView {
    fn new(_: &mut Context<Self>) -> Self {
        Self {
            catalog: catalog(),
            problems: Vec::new(),
            fetching: false,
            _task: None,
        }
    }

    /// Fetches the catalog repository again and checks its signature; keeps
    /// the old list on failure.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let config = catalog_config();
        let Some(url) = config.effective_url() else {
            cx.notify();
            return;
        };
        self.fetching = true;
        cx.notify();
        let fetch = cx.background_executor().spawn(async move {
            let files = fetch_catalog(&url, &FetchControl::default())?;
            let (catalog, problems) = Catalog::load(&files, &config.keys)?;
            Ok::<_, String>((files, catalog, problems))
        });
        self._task = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            _ = this.update(cx, |this, cx| {
                this.fetching = false;
                match result {
                    Ok((files, catalog, problems)) => {
                        let catalog = Arc::new(catalog);
                        remember(&files, catalog.clone());
                        this.catalog = Some(catalog);
                        this.problems = problems;
                    }
                    Err(err) => this.problems = vec![err],
                }
                cx.notify();
            });
        }));
    }

    /// Where the catalog comes from and whether its signature was checked.
    fn header(&self, cx: &App) -> Option<impl IntoElement> {
        let muted = cx.theme().muted_foreground;
        let url = catalog_config().effective_url()?;
        let signing = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.signing.clone());
        let signing = signing.map(|signing| match signing {
            Signing::Trusted { key, comment } => (
                Tag::success().outline().small().child("Signed"),
                // minisign separates the trusted comment's fields with tabs.
                format!(
                    "by trusted key {key}… · {}",
                    comment.split('\t').collect::<Vec<_>>().join(" · ")
                ),
            ),
            Signing::NotChecked => (
                Tag::secondary().outline().small().child("Not checked"),
                "No trusted keys are set (Source…), so its signature is not checked.".into(),
            ),
        });
        Some(
            v_flex()
                .gap_1()
                .pb(fixed::PANEL_GAP)
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis_start()
                        .child(format!("From {url}")),
                )
                .when_some(signing, |header, (tag, text)| {
                    header.child(
                        h_flex()
                            .gap_1()
                            .child(tag)
                            .child(div().text_xs().text_color(muted).child(text)),
                    )
                }),
        )
    }

    fn row(&self, listed: &CatalogPackage, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let registry = workflows::registry();
        let latest = listed.best(None);
        let installed = registry.get(&listed.id).map(|def| {
            let ours = matches!(registry.origin(&listed.id), Some(Origin::Installed(_)));
            (def.version.clone(), ours)
        });
        let action = latest.and_then(|latest| match &installed {
            None => Some("Install"),
            Some((version, true)) => {
                let newer = semver::Version::parse(&latest.version).ok()
                    > semver::Version::parse(version).ok();
                newer.then_some("Update")
            }
            // A user or built-in workflow has the id: installing would clash.
            Some((_, false)) => None,
        });
        let icon = workflows::named_icon(listed.icon.as_deref())
            .unwrap_or_else(|| Icon::new(IconName::LayoutDashboard));
        let signed = self
            .catalog
            .as_ref()
            .is_some_and(|catalog| catalog.is_signed());
        let install = action.map(|label| {
            let listed = listed.clone();
            Button::new(SharedString::from(format!("catalog-install-{}", listed.id)))
                .label(label)
                .with_size(Size::Small)
                .when(label == "Install", |button| button.primary())
                .when(label == "Update", |button| button.outline())
                .disabled(workflows::dirs().is_none())
                .on_click(move |_, window, cx| {
                    install_from_catalog(listed.clone(), None, signed, window, cx)
                })
        });
        h_flex()
            .w_full()
            .gap(fixed::PANEL_GAP)
            .py(fixed::PANEL_GAP)
            .border_b_1()
            .border_color(border)
            .child(icon.with_size(Size::Size(fixed::SECTION_ICON_SIZE)))
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
                                    .child(listed.name.clone()),
                            )
                            .when_some(installed.as_ref(), |row, (version, _)| {
                                row.child(
                                    Tag::secondary()
                                        .outline()
                                        .small()
                                        .child(format!("Installed {version}")),
                                )
                            }),
                    )
                    .child(div().text_xs().text_color(muted).child(format!(
                        "{} · {}",
                        listed.id,
                        latest.map_or("", |latest| latest.version.as_str())
                    )))
                    .when(!listed.description.is_empty(), |column| {
                        column.child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(listed.description.clone()),
                        )
                    }),
            )
            .children(install)
    }
}

impl Render for CatalogView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let note = |text: String| div().text_sm().text_color(muted).child(text);
        let rows: Vec<AnyElement> = self
            .catalog
            .iter()
            .flat_map(|catalog| catalog.packages.iter())
            .map(|listed| self.row(listed, cx).into_any_element())
            .collect();
        let url = catalog_config().effective_url();
        v_flex()
            .id("workflow-catalog")
            .h(fixed::MANAGE_BODY_HEIGHT)
            .overflow_y_scroll()
            .children(self.header(cx))
            .map(|list| match (url, &self.catalog) {
                (None, _) => list.child(note(
                    "No catalog is set. Use Source… to name a git repository whose root \
                     holds catalog.json."
                        .into(),
                )),
                (_, None) if self.fetching => list.child(note("Fetching the catalog…".into())),
                (_, Some(catalog)) if catalog.packages.is_empty() => {
                    list.child(note("The catalog lists no packages.".into()))
                }
                _ => list,
            })
            .children(rows)
            .children(self.problems.iter().map(|problem| {
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(danger)
                    .child(problem.clone())
            }))
    }
}
