//! Manage dialogs, opened from the title bar's app menu (the three lines),
//! File > Preferences and the command palette: Extensions, Settings. Contents
//! are placeholders for now.

use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, WindowExt, v_flex};
use gpui_kit::*;

use crate::menus::{ManageExtensions, ManageSettings};
use crate::style::fixed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manage {
    Extensions,
    Settings,
}

impl Manage {
    /// In menu order.
    pub const ALL: [Manage; 2] = [Manage::Extensions, Manage::Settings];

    /// Menu item and dialog title.
    pub fn label(self) -> &'static str {
        match self {
            Manage::Extensions => "Extensions",
            Manage::Settings => "Settings",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Manage::Extensions => IconName::Puzzle,
            Manage::Settings => IconName::Settings,
        }
    }

    /// The action that opens this dialog.
    pub fn action(self) -> Box<dyn Action> {
        match self {
            Manage::Extensions => Box::new(ManageExtensions),
            Manage::Settings => Box::new(ManageSettings),
        }
    }
}

/// Opens `target`'s dialog: title and an empty placeholder body.
pub fn open(target: Manage, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        dialog.title(target.label()).w(fixed::MANAGE_WIDTH).child(
            v_flex()
                .h(fixed::MANAGE_BODY_HEIGHT)
                .items_center()
                .justify_center()
                .gap(fixed::PANEL_GAP)
                .text_color(muted)
                .child(
                    Icon::new(target.icon()).with_size(Size::Size(fixed::MANAGE_EMPTY_ICON_SIZE)),
                )
                .child(div().text_sm().child("Nothing here yet.")),
        )
    });
}
