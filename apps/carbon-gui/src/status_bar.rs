//! Compact bottom bar, left to right: workspace, git branch, jobs (DESIGN.md s4);
//! language servers on the right.
//! Reads `carbon_core::Core`; the branch is polled because nothing watches `.git` yet.

use std::time::Duration;

use carbon_core::lsp::{LspStatus, ServerEntry};
use carbon_core::{Branch, Core};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, h_flex};
use gpui_kit::*;

use crate::manage::Manage;
use crate::menus::{CloseFolder, CommandPalette, CreateFolder, OpenFolder};
use crate::style::fixed;

/// How often the branch is re-read from `.git/HEAD` (a tiny file read).
const BRANCH_POLL: Duration = Duration::from_secs(2);

pub struct CarbonStatusBar {
    core: Entity<Core>,
    branch: Option<Branch>,
    _core_sub: Subscription,
    _host_sub: Subscription,
    _branch_poll: Task<()>,
}

impl CarbonStatusBar {
    pub fn new(core: Entity<Core>, cx: &mut Context<Self>) -> Self {
        let branch = core.read(cx).branch();
        // Jobs or workspace changed: the core entity was notified.
        let _core_sub = cx.observe(&core, |this, _, cx| {
            this.refresh_branch(cx);
            cx.notify();
        });
        // Questions from extension runs and agents.
        let _host_sub =
            cx.observe_global::<crate::extensions::host::AgentsHost>(|_, cx| cx.notify());
        let _branch_poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BRANCH_POLL).await;
                if this.update(cx, |this, cx| this.refresh_branch(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            core,
            branch,
            _core_sub,
            _host_sub,
            _branch_poll,
        }
    }

    fn refresh_branch(&mut self, cx: &mut Context<Self>) {
        let branch = self.core.read(cx).branch();
        if branch != self.branch {
            self.branch = branch;
            cx.notify();
        }
    }
}

/// Icon + text, one status bar entry.
fn item(icon: IconName, text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    h_flex()
        .gap(fixed::STATUS_ITEM_GAP)
        .child(
            Icon::new(icon)
                .with_size(Size::Size(fixed::STATUS_ICON_SIZE))
                .text_color(cx.theme().muted_foreground),
        )
        .child(text.into())
}

/// Workspace name; click for Open / Create / Close Folder (menu opens upward).
fn workspace_button(name: SharedString, has_workspace: bool, cx: &App) -> impl IntoElement {
    Button::new("status-workspace")
        .ghost()
        .xsmall()
        .child(item(IconName::Folder, name, cx))
        .tooltip("Open, create or close a folder")
        .dropdown_menu_with_anchor(Anchor::BottomLeft, move |menu, _, _| {
            let menu = menu
                .menu("Open Folder...", Box::new(OpenFolder))
                .menu("Create Folder...", Box::new(CreateFolder));
            if has_workspace {
                menu.separator().menu("Close Folder", Box::new(CloseFolder))
            } else {
                menu
            }
        })
}

/// One language server: its name and state; hover for details. A missing
/// server its extension can download offers that on click.
fn language_server(
    ix: usize,
    entry: &ServerEntry,
    core: &Entity<Core>,
    cx: &App,
) -> impl IntoElement {
    let name = entry.config.name.clone();
    let download = match (&entry.status, &entry.config.download) {
        (LspStatus::Missing(_), Some(download)) => Some(download),
        _ => None,
    };
    let (text, tooltip): (String, Option<String>) = match &entry.status {
        LspStatus::Starting => (format!("{name}: starting"), None),
        LspStatus::Busy(work) => (format!("{name}: {work}"), None),
        LspStatus::Ready => (name.to_string(), None),
        LspStatus::Missing(hint) => match download {
            Some(download) => (
                format!("{name}: not installed"),
                Some(format!(
                    "{hint}
Click to download version {} from {}.",
                    download.version, download.host
                )),
            ),
            None => (format!("{name}: not installed"), Some(hint.clone())),
        },
        LspStatus::Failed(error) => (format!("{name}: stopped"), Some(error.clone())),
    };
    let mut button =
        Button::new(("status-lsp", ix))
            .ghost()
            .xsmall()
            .child(item(IconName::Code, text, cx));
    if download.is_some() {
        let (core, language) = (core.clone(), entry.config.language.clone());
        button = button.on_click(move |_, window, cx| {
            crate::extensions::ask_download(core.clone(), language.clone(), window, cx)
        });
    }
    match tooltip {
        Some(tooltip) => button.tooltip(tooltip),
        None => button,
    }
}

/// The three-line button at the bar's right end: extensions, settings and
/// the palette in one place (menu opens upward).
fn app_menu_button() -> impl IntoElement {
    Button::new("app-menu")
        .ghost()
        .xsmall()
        .icon(Icon::new(IconName::Menu).with_size(Size::Size(fixed::APP_MENU_ICON_SIZE)))
        .tooltip("Manage")
        .accessibility_label("Manage")
        .dropdown_menu_with_anchor(Anchor::BottomRight, |menu, _, _| {
            Manage::ALL
                .into_iter()
                .fold(menu, |menu, target| {
                    menu.menu_with_icon(target.label(), target.icon(), target.action())
                })
                .separator()
                .menu_with_icon(
                    "Command Palette",
                    IconName::Search,
                    Box::new(CommandPalette),
                )
        })
}

impl Render for CarbonStatusBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let core = self.core.read(cx);
        let has_workspace = core.workspace().is_some();
        let workspace = core
            .workspace()
            .map(|w| SharedString::from(w.name().to_string()))
            .unwrap_or_else(|| "No folder".into());
        let running: Vec<_> = core.jobs.running().map(|job| job.title.clone()).collect();
        let jobs = match running.as_slice() {
            [] => "No jobs".to_string(),
            [title] => title.clone(),
            many => format!("{} jobs", many.len()),
        };

        // Same height as the title/menu bar and the tab bar.
        let mut bar = StatusBar::new()
            .h(fixed::BAR_HEIGHT)
            .py_0()
            .px(fixed::STATUS_BAR_PADDING)
            .left(workspace_button(workspace, has_workspace, cx));
        if let Some(branch) = &self.branch {
            bar = bar.left(item(IconName::GitBranch, branch.label().to_string(), cx));
        }
        bar = bar.left(item(IconName::ListChecks, jobs, cx));
        // Stop for jobs extension code started (an agent flow's run).
        let stoppable: Vec<(u64, String)> = core
            .jobs
            .running()
            .filter_map(|job| {
                crate::extensions::host::job_for(job.id(), cx).map(|ext| (ext, job.title.clone()))
            })
            .take(3)
            .collect();
        for (ix, (job, title)) in stoppable.into_iter().enumerate() {
            bar = bar.left(
                Button::new(("status-stop-job", ix))
                    .icon(IconName::Square)
                    .ghost()
                    .xsmall()
                    .tooltip(format!("Stop {title}"))
                    .on_click(move |_, _, cx| crate::extensions::host::stop_job(job, cx)),
            );
        }
        if let Some(waiting) = crate::extensions::host::waiting_summary(cx) {
            bar = bar.left(
                Button::new("status-waiting")
                    .ghost()
                    .xsmall()
                    .child(
                        h_flex()
                            .gap(fixed::STATUS_ITEM_GAP)
                            .text_color(cx.theme().warning)
                            .child(
                                Icon::new(IconName::TriangleAlert)
                                    .with_size(Size::Size(fixed::STATUS_ICON_SIZE)),
                            )
                            .child(waiting.label.clone()),
                    )
                    .tooltip("An agent flow waits for your answer")
                    .on_click(move |_, window, cx| {
                        if let Some(path) = &waiting.path {
                            crate::graph_panel::open_graph_file(path, window, cx);
                        }
                    }),
            );
        }
        let mut servers: Vec<_> = core.lsp.entries().collect();
        servers.sort_by(|a, b| a.config.name.cmp(&b.config.name));
        for (ix, entry) in servers.into_iter().enumerate() {
            bar = bar.right(language_server(ix, entry, &self.core, cx));
        }
        bar.right(app_menu_button())
    }
}
