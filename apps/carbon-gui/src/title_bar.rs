//! Custom window title bar: logo, menu bar, window controls (drawn by `TitleBar`).

use gpui_kit::component::menu::AppMenuBar;
use gpui_kit::component::{ActiveTheme, TitleBar, h_flex};
use gpui_kit::*;

use crate::style::fixed;

pub struct AppTitleBar {
    menu_bar: Entity<AppMenuBar>,
}

impl AppTitleBar {
    pub fn new(cx: &mut App) -> Self {
        Self {
            menu_bar: AppMenuBar::new(cx),
        }
    }

    /// Re-reads the app menus after `menus::refresh`.
    pub fn reload_menus(&self, cx: &mut Context<Self>) {
        self.menu_bar.update(cx, |menu_bar, cx| menu_bar.reload(cx));
    }
}

/// Placeholder logo: a letter on the theme's primary colour.
fn logo(cx: &App) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(fixed::LOGO_SIZE)
        .rounded(cx.theme().radius)
        .bg(cx.theme().primary)
        .text_color(cx.theme().primary_foreground)
        .text_sm()
        .font_weight(fixed::LOGO_WEIGHT)
        .child(fixed::LOGO_TEXT)
}

impl Render for AppTitleBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row = h_flex().h_full().gap(fixed::TITLE_BAR_GAP).child(logo(cx));

        // macOS uses the native menu bar (set in `menus::init`).
        #[cfg(not(target_os = "macos"))]
        let row = row.child(self.menu_bar.clone());

        // The x asks about unsaved changes first (the kit's default just
        // closes the window).
        TitleBar::new()
            .on_close_window(|_, window, cx| crate::save_ops::request_quit(window, cx))
            .child(row)
    }
}
