//! Manage dialogs opened from the bottom of the Section Bar: Manage Extensions,
//! Manage Workflows, Manage Settings. Contents are placeholders for now.

use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, WindowExt, v_flex};
use gpui_kit::*;

use crate::style::fixed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manage {
    Extensions,
    Workflows,
    Settings,
}

impl Manage {
    /// Top to bottom, as shown in the Section Bar.
    pub const ALL: [Manage; 3] = [Manage::Extensions, Manage::Workflows, Manage::Settings];

    /// Button tooltip and dialog title.
    pub fn label(self) -> &'static str {
        match self {
            Manage::Extensions => "Manage Extensions",
            Manage::Workflows => "Manage Workflows",
            Manage::Settings => "Manage Settings",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Manage::Extensions => IconName::Puzzle,
            Manage::Workflows => IconName::Workflow,
            Manage::Settings => IconName::Settings,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Manage::Extensions => "manage-extensions",
            Manage::Workflows => "manage-workflows",
            Manage::Settings => "manage-settings",
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
