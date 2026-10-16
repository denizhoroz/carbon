//! Explorer panel. Empty state: Open Folder / Create Folder (DESIGN.md s4:
//! "no workspace = Open and Create buttons"). With a folder open it shows the
//! folder for now; the file tree is not built yet.

use carbon_core::Core;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, panel_handle, register_panel};
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::*;

use crate::menus::{CreateFolder, OpenFolder};
use crate::style::fixed;
use crate::workspace_ops::current_core;

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "Explorer";

pub struct ExplorerPanel {
    core: Entity<Core>,
    focus_handle: FocusHandle,
    _core_sub: Subscription,
}

impl ExplorerPanel {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let core = current_core(cx);
        cx.new(|cx| Self {
            _core_sub: cx.observe(&core, |_, _, cx| cx.notify()),
            core,
            focus_handle: cx.focus_handle(),
        })
    }
}

/// Lets saved layouts recreate the Explorer.
pub fn register(cx: &mut App) {
    register_panel(cx, PANEL_NAME, |_, _, cx| {
        panel_handle(ExplorerPanel::new(cx))
    });
}

impl EventEmitter<PanelEvent> for ExplorerPanel {}

impl Focusable for ExplorerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ExplorerPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }
}

impl Panel for ExplorerPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("Explorer".into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Explorer"
    }
}

impl Render for ExplorerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let body = v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .p(fixed::PANEL_PADDING)
            .gap(fixed::PANEL_GAP);

        let Some(workspace) = self.core.read(cx).workspace() else {
            return body
                .items_center()
                .justify_center()
                .text_center()
                .child("No folder open")
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("Open a folder or create a new one to start."),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .justify_center()
                        .child(
                            Button::new("explorer-open-folder")
                                .primary()
                                .label("Open Folder")
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(OpenFolder), cx)
                                }),
                        )
                        .child(
                            Button::new("explorer-create-folder")
                                .label("Create Folder")
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(CreateFolder), cx)
                                }),
                        ),
                );
        };

        body.child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(workspace.name().to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(muted)
                // Paths: keep the end (the folder) visible.
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_start()
                .child(workspace.root().display().to_string()),
        )
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .child("File tree is not built yet."),
        )
    }
}
