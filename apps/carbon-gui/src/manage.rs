//! Manage dialogs, opened from the status bar's app menu (the three lines),
//! File > Preferences and the command palette: Extensions (`extensions/manage.rs`)
//! and Settings (`settings.rs`).

use gpui_kit::assets::IconName;
use gpui_kit::*;

use crate::menus::{ManageExtensions, ManageSettings};

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

/// Opens `target`'s dialog.
pub fn open(target: Manage, window: &mut Window, cx: &mut App) {
    match target {
        Manage::Settings => crate::settings::open_dialog(window, cx),
        Manage::Extensions => {
            crate::extensions::open_manage(crate::extensions::Tab::Catalog, window, cx)
        }
    }
}
