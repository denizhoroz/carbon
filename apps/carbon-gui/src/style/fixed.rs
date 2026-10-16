//! Fixed styling choices: not part of any theme and not user-configurable.
//! Colours, fonts and radii belong in theme JSON, not here.

use gpui_kit::{FontWeight, Pixels, Rems, Size, px, rems, size};

/// Initial window size (centered on the main display).
pub const WINDOW_SIZE: Size<Pixels> = size(px(1280.), px(800.));

/// Placeholder logo: letter, square size, weight.
pub const LOGO_TEXT: &str = "C";
pub const LOGO_SIZE: Rems = rems(1.25);
pub const LOGO_WEIGHT: FontWeight = FontWeight::BOLD;

/// Gap between items in the title bar row (logo, menu bar).
pub const TITLE_BAR_GAP: Rems = rems(0.5);

/// Default dock sizes (width for left/right, height for bottom); users resize from here.
pub const LEFT_DOCK_SIZE: Pixels = px(240.);
pub const RIGHT_DOCK_SIZE: Pixels = px(320.);
pub const BOTTOM_DOCK_SIZE: Pixels = px(200.);

/// Section Bar (left icon strip): width, padding at its top/bottom edge, spacing between icons.
pub const SECTION_BAR_WIDTH: Pixels = px(52.);
pub const SECTION_BAR_PADDING: Rems = rems(0.5);
/// Section Bar button (square) and its icon; (52 - 36) / 2 = 8 px clear on each side.
pub const SECTION_BUTTON_SIZE: Pixels = px(36.);
pub const SECTION_ICON_SIZE: Pixels = px(20.);
pub const SECTION_BAR_GAP: Rems = rems(0.25);

/// Status bar (bottom): side padding, icon size, icon-to-text gap.
pub const STATUS_BAR_PADDING: Rems = rems(0.75);
pub const STATUS_ICON_SIZE: Pixels = px(14.);
pub const STATUS_ITEM_GAP: Rems = rems(0.25);

/// Width of small dialogs (Create Folder).
pub const DIALOG_WIDTH: Pixels = px(480.);

/// Inner padding of panel bodies (DESIGN.md s4: 16 px) and gap between their items.
pub const PANEL_PADDING: Rems = rems(1.);
pub const PANEL_GAP: Rems = rems(0.5);

/// Keyboard resize step and the smallest size it shrinks a dock to.
pub const DOCK_RESIZE_STEP: Pixels = px(24.);
pub const DOCK_MIN_SIZE: Pixels = px(120.);

/// Explorer file tree: row height (dense, like VS Code's 22 px), indent per
/// level, icon size, side padding, and the gap between chevron, icon and name.
pub const TREE_ROW_HEIGHT: Pixels = px(24.);
pub const TREE_INDENT: Pixels = px(12.);
pub const TREE_ICON_SIZE: Pixels = px(16.);
pub const TREE_PADDING: Pixels = px(8.);
pub const TREE_ITEM_GAP: Pixels = px(4.);
/// Explorer markers: agent-edit icon and the dot a folder shows for what is inside.
pub const TREE_MARK_ICON_SIZE: Pixels = px(12.);
pub const TREE_DOT_SIZE: Pixels = px(6.);
