//! Fixed styling choices: not part of any theme and not user-configurable.
//! Colours, fonts and radii belong in theme JSON, not here.

use std::time::Duration;

use gpui_kit::{FontWeight, Pixels, Rems, Size, px, rems, size};

/// Initial window size (centered on the main display).
pub const WINDOW_SIZE: Size<Pixels> = size(px(1280.), px(800.));

/// Placeholder logo: letter, square size, weight.
pub const LOGO_TEXT: &str = "C";
pub const LOGO_SIZE: Rems = rems(1.25);
pub const LOGO_WEIGHT: FontWeight = FontWeight::BOLD;

/// Height of the title (menu) bar and the status bar: the editor tab bar's
/// height (vendored kit tabs are 30 px), so the three bars line up.
pub const BAR_HEIGHT: Pixels = px(30.);

/// Gap between items in the title bar row (logo, menu bar).
pub const TITLE_BAR_GAP: Rems = rems(0.5);

/// Command center (title bar search box that opens the palette): share of
/// the window width, its limits, the room kept free on each side for the
/// menus and the buttons, height, icon size, gap between its parts.
pub const COMMAND_CENTER_SHARE: f32 = 0.32;
pub const COMMAND_CENTER_MAX_WIDTH: Pixels = px(520.);
pub const COMMAND_CENTER_MIN_WIDTH: Pixels = px(200.);
pub const COMMAND_CENTER_CLEARANCE: Pixels = px(300.);
pub const COMMAND_CENTER_HEIGHT: Pixels = px(22.);
pub const COMMAND_CENTER_ICON_SIZE: Pixels = px(14.);
pub const COMMAND_CENTER_GAP: Rems = rems(0.375);

/// Icon of the status bar's app menu button (three lines).
pub const APP_MENU_ICON_SIZE: Pixels = px(16.);

/// Default dock sizes (width for left/right, height for bottom); users resize from here.
pub const LEFT_DOCK_SIZE: Pixels = px(240.);
pub const RIGHT_DOCK_SIZE: Pixels = px(320.);
pub const BOTTOM_DOCK_SIZE: Pixels = px(200.);

/// Status bar (bottom): side padding, icon size, icon-to-text gap.
pub const STATUS_BAR_PADDING: Rems = rems(0.75);
pub const STATUS_ICON_SIZE: Pixels = px(14.);
pub const STATUS_ITEM_GAP: Rems = rems(0.25);

/// Width of small dialogs (Create Folder).
pub const DIALOG_WIDTH: Pixels = px(480.);

/// Manage dialogs (Extensions, Settings): width, body height,
/// placeholder icon size.
pub const MANAGE_WIDTH: Pixels = px(720.);
pub const MANAGE_BODY_HEIGHT: Pixels = px(420.);
/// Icon of an extension in Manage Extensions and the install dialog.
pub const EXTENSION_ICON_SIZE: Pixels = px(20.);

/// Settings dialog: share of the window and limits of its size, the category
/// tree's width, padding of the top bar and of setting rows, the gap between a
/// row's text and its control, control widths (text, number, choice), and the
/// tree's chevrons.
pub const SETTINGS_WIDTH_SHARE: f32 = 0.8;
pub const SETTINGS_MAX_WIDTH: Pixels = px(1100.);
pub const SETTINGS_MIN_WIDTH: Pixels = px(640.);
pub const SETTINGS_HEIGHT_SHARE: f32 = 0.85;
pub const SETTINGS_MAX_HEIGHT: Pixels = px(760.);
pub const SETTINGS_MIN_HEIGHT: Pixels = px(420.);
/// Room the dialog's title takes above the settings body.
pub const SETTINGS_TITLE_HEIGHT: Pixels = px(52.);
pub const SETTINGS_TREE_WIDTH: Pixels = px(220.);
pub const SETTINGS_BAR_PADDING: Pixels = px(12.);
pub const SETTINGS_ROW_PADDING_X: Pixels = px(20.);
pub const SETTINGS_ROW_PADDING_Y: Pixels = px(10.);
pub const SETTINGS_ROW_GAP: Pixels = px(24.);
pub const SETTINGS_TEXT_WIDTH: Pixels = px(260.);
pub const SETTINGS_NUMBER_WIDTH: Pixels = px(120.);
pub const SETTINGS_CHOICE_MIN_WIDTH: Pixels = px(180.);
pub const SETTINGS_CHEVRON_SIZE: Pixels = px(14.);

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

/// Terminal: line height as a multiple of the font size, the left gutter
/// (command marks live there), top padding, scrollbar width, mark size.
pub const TERMINAL_LINE_HEIGHT: f32 = 1.3;
pub const TERMINAL_GUTTER: Pixels = px(14.);
pub const TERMINAL_PADDING_TOP: Pixels = px(4.);
pub const TERMINAL_SCROLLBAR_WIDTH: Pixels = px(6.);
pub const TERMINAL_MARK_SIZE: Pixels = px(6.);

/// Command palette (VS Code / Zed style): width, gap below the title bar, the
/// least room kept to the window edge, list height before it scrolls, row
/// icon size, and the open animation (fade plus a short settle downwards).
pub const PALETTE_WIDTH: Pixels = px(600.);
pub const PALETTE_TOP_GAP: Pixels = px(6.);
pub const PALETTE_WINDOW_MARGIN: Pixels = px(16.);
pub const PALETTE_LIST_MAX_HEIGHT: Pixels = px(420.);
pub const PALETTE_ICON_SIZE: Pixels = px(16.);
pub const PALETTE_FADE: Duration = Duration::from_millis(150);
pub const PALETTE_FADE_TRAVEL: Pixels = px(4.);

/// Chat panel: side padding, padding inside the user's message box, the readable column width (centred in wide panels),
/// starter-prompt width, chip width, role and empty-state icons, composer
/// rows, and how long a streaming reply waits before it is saved.
pub const CHAT_PADDING: Pixels = px(12.);
pub const CHAT_BUBBLE_PADDING: Pixels = px(8.);
pub const CHAT_COLUMN_MAX_WIDTH: Pixels = px(760.);
pub const CHAT_STARTERS_MAX_WIDTH: Pixels = px(320.);
pub const CHAT_CHIP_MAX_WIDTH: Pixels = px(220.);
pub const CHAT_ROLE_ICON_SIZE: Pixels = px(12.);
pub const CHAT_EMPTY_ICON_SIZE: Pixels = px(28.);
pub const CHAT_COMPOSER_MIN_ROWS: usize = 2;
pub const CHAT_COMPOSER_MAX_ROWS: usize = 10;
pub const CHAT_SAVE_DELAY: Duration = Duration::from_secs(1);
