//! Custom window title bar: logo, menu bar, the command center (a search box
//! that opens the command palette, like VS Code's), window controls (drawn by
//! `TitleBar`).

use gpui_kit::assets::IconName;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::menu::AppMenuBar;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, TitleBar, h_flex};
use gpui_kit::*;

use crate::menus::CommandPalette;
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

/// Width of the command center in a window `viewport` wide: a share of the
/// window, kept clear of the menus on the left and the buttons on the right;
/// `None` when the window is too narrow for it.
fn command_center_width(viewport: Pixels) -> Option<Pixels> {
    let width = (viewport * fixed::COMMAND_CENTER_SHARE)
        .min(fixed::COMMAND_CENTER_MAX_WIDTH)
        .min(viewport - fixed::COMMAND_CENTER_CLEARANCE * 2.);
    (width >= fixed::COMMAND_CENTER_MIN_WIDTH).then_some(width)
}

/// The always-visible way into the command palette: a search box centered on
/// the window (where the palette opens); click it or press the shortcut.
fn command_center(width: Pixels, window: &Window, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let shortcut = Kbd::binding_for_action(&CommandPalette, None, window);
    h_flex()
        .id("command-center")
        .w(width)
        .h(fixed::COMMAND_CENTER_HEIGHT)
        .justify_center()
        .gap(fixed::COMMAND_CENTER_GAP)
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .text_xs()
        .text_color(theme.muted_foreground)
        .cursor_pointer()
        .hover(|this| this.bg(theme.secondary_hover))
        // It lies over the title bar, whose mouse-down starts a window drag:
        // without this a real click moved the window instead of opening the
        // palette.
        .occlude()
        .child(Icon::new(IconName::Search).with_size(Size::Size(fixed::COMMAND_CENTER_ICON_SIZE)))
        .child("Search commands and files")
        .children(shortcut)
        .on_click(|_, window, cx| window.dispatch_action(Box::new(CommandPalette), cx))
}

impl Render for AppTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left = h_flex().h_full().gap(fixed::TITLE_BAR_GAP).child(logo(cx));

        // macOS uses the native menu bar (set in `menus::init`).
        #[cfg(not(target_os = "macos"))]
        let left = left.child(self.menu_bar.clone());

        // Centered on the window, not on the space between the menus and the
        // window controls, so it sits where the palette opens.
        let viewport = window.viewport_size().width;
        let center = command_center_width(viewport).map(|width| {
            div()
                .absolute()
                .top_0()
                .left((viewport - width) / 2.)
                .h(fixed::BAR_HEIGHT)
                .flex()
                .items_center()
                .child(command_center(width, window, cx))
        });

        // The x asks about unsaved changes first (the kit's default just
        // closes the window).
        div()
            .relative()
            .flex_none()
            .child(
                TitleBar::new()
                    .h(fixed::BAR_HEIGHT)
                    .on_close_window(|_, window, cx| crate::save_ops::request_quit(window, cx))
                    .child(left),
            )
            .children(center)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;

    use super::command_center_width;
    use crate::style::fixed;

    #[test]
    fn command_center_fits_the_window() {
        // A wide window caps it; a medium one takes its share.
        assert_eq!(
            command_center_width(px(2400.)),
            Some(fixed::COMMAND_CENTER_MAX_WIDTH)
        );
        let medium = command_center_width(px(1280.)).unwrap();
        assert!(medium < fixed::COMMAND_CENTER_MAX_WIDTH && medium > px(300.));
        // A narrow one has no room left between the menus and the buttons.
        assert_eq!(command_center_width(px(600.)), None);
    }
}
