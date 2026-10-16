//! Compact bottom bar, left to right: workspace, git branch, jobs (DESIGN.md s4).
//! Reads `carbon_core::Core`; the branch is polled because nothing watches `.git` yet.

use std::time::Duration;

use carbon_core::{Branch, Core};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, TITLE_BAR_HEIGHT, h_flex};
use gpui_kit::*;

use crate::menus::{CloseFolder, CreateFolder, OpenFolder};
use crate::style::fixed;

/// How often the branch is re-read from `.git/HEAD` (a tiny file read).
const BRANCH_POLL: Duration = Duration::from_secs(2);

pub struct CarbonStatusBar {
    core: Entity<Core>,
    branch: Option<Branch>,
    _core_sub: Subscription,
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

        // Same height as the title/menu bar (kit constant, so they stay equal).
        let mut bar = StatusBar::new()
            .h(TITLE_BAR_HEIGHT)
            .py_0()
            .px(fixed::STATUS_BAR_PADDING)
            .left(workspace_button(workspace, has_workspace, cx));
        if let Some(branch) = &self.branch {
            bar = bar.left(item(IconName::GitBranch, branch.label().to_string(), cx));
        }
        bar.left(item(IconName::ListChecks, jobs, cx))
    }
}
