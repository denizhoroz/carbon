//! Placeholder dock panel for testing layout: shows its title, restores from a saved layout.

use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::*;

use crate::style::fixed;

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "TestPanel";

pub struct TestPanel {
    title: SharedString,
    focus_handle: FocusHandle,
}

impl TestPanel {
    pub fn new(title: impl Into<SharedString>, cx: &mut App) -> Entity<Self> {
        let title = title.into();
        cx.new(|cx| Self {
            title,
            focus_handle: cx.focus_handle(),
        })
    }
}

/// Registers the builder that recreates test panels when a saved layout loads.
pub fn register(cx: &mut App) {
    register_panel(cx, PANEL_NAME, |build, _, cx| {
        let title = match build.info() {
            PanelInfo::Panel(value) => value["title"].as_str().unwrap_or("Panel").to_string(),
            _ => "Panel".to_string(),
        };
        panel_handle(TestPanel::new(title, cx))
    });
}

impl EventEmitter<PanelEvent> for TestPanel {}

impl Focusable for TestPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for TestPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        state.info = PanelInfo::Panel(serde_json::json!({ "title": self.title.as_ref() }));
        state
    }
}

impl Panel for TestPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some(self.title.clone())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.title.clone()
    }
}

impl Render for TestPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .p(fixed::PANEL_PADDING)
            .items_center()
            .justify_center()
            .text_center()
            .gap(fixed::PANEL_GAP)
            .child(self.title.clone())
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Drag the tab to move or split; drag edges to resize."),
            )
    }
}
