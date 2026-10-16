//! Editor panel placeholder: shows the file the Explorer opened (the core's
//! active buffer) until the real editor is built.

use carbon_core::Core;
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, panel_handle, register_panel};
use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::*;

use crate::style::fixed;
use crate::workspace_ops::current_core;

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "Editor";

pub struct EditorPanel {
    core: Entity<Core>,
    focus_handle: FocusHandle,
    _core_sub: Subscription,
}

impl EditorPanel {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let core = current_core(cx);
        cx.new(|cx| Self {
            _core_sub: cx.observe(&core, |_, _, cx| cx.notify()),
            core,
            focus_handle: cx.focus_handle(),
        })
    }
}

/// Lets saved layouts recreate the Editor.
pub fn register(cx: &mut App) {
    register_panel(cx, PANEL_NAME, |_, _, cx| {
        panel_handle(EditorPanel::new(cx))
    });
}

impl EventEmitter<PanelEvent> for EditorPanel {}

impl Focusable for EditorPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for EditorPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }
}

impl Panel for EditorPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("Editor".into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Editor"
    }
}

impl Render for EditorPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let body = v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .p(fixed::PANEL_PADDING)
            .gap(fixed::PANEL_GAP)
            .items_center()
            .justify_center()
            .text_center();

        let core = self.core.read(cx);
        let Some(buffer) = core.active_buffer() else {
            return body.child("Editor").child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Pick a file in the Explorer."),
            );
        };
        let path = buffer.path();
        let name = path
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        let relative = path
            .and_then(|path| core.workspace().and_then(|ws| ws.relative(path)))
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let lines = match buffer.line_count() {
            1 => "1 line".to_string(),
            n => format!("{n} lines"),
        };
        // The Explorer can delete an open file; the buffer stays (like VS Code).
        let deleted = path.is_some_and(|path| !path.exists());
        body.child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
            "Opened: {name}{}",
            if deleted { " (deleted)" } else { "" }
        )))
        .child(div().text_sm().text_color(muted).child(relative))
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .child(format!("{lines}. The editor is not built yet.")),
        )
    }
}
