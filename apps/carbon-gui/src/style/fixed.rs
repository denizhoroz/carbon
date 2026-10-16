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

/// Keyboard resize step and the smallest size it shrinks a dock to.
pub const DOCK_RESIZE_STEP: Pixels = px(24.);
pub const DOCK_MIN_SIZE: Pixels = px(120.);
