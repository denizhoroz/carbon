//! Layout tier T2 data: built-in task presets and the user's named layouts.
//! Named layouts and auto-hide flags live in `layouts.json` beside the current layout.

use gpui_kit::component::dock::{DockAreaState, DockLayout, DockPlacement, panel_handle};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

use crate::editor_panel::EditorPanel;
use crate::explorer_panel::ExplorerPanel;
use crate::style::fixed;
use crate::terminal_panel::TerminalPanel;
use crate::test_panel::TestPanel;

const STORE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/carbon-gui");
const STORE_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/carbon-gui/layouts.json"
);

/// Named layouts map to Ctrl+Alt+1..9, so at most nine.
pub const MAX_SAVED: usize = 9;

/// A dock edge (the center is not a dock).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 3] = [Side::Left, Side::Right, Side::Bottom];

    pub fn placement(self) -> DockPlacement {
        match self {
            Side::Left => DockPlacement::Left,
            Side::Right => DockPlacement::Right,
            Side::Bottom => DockPlacement::Bottom,
        }
    }

    pub fn of(placement: DockPlacement) -> Option<Side> {
        Side::ALL
            .into_iter()
            .find(|side| side.placement() == placement)
    }

    /// Size of a dock created on this side.
    pub fn default_size(self) -> Pixels {
        match self {
            Side::Left => fixed::LEFT_DOCK_SIZE,
            Side::Right => fixed::RIGHT_DOCK_SIZE,
            Side::Bottom => fixed::BOTTOM_DOCK_SIZE,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Side::Left => "Left",
            Side::Right => "Right",
            Side::Bottom => "Bottom",
        }
    }
}

/// Built-in task layouts (Eclipse perspectives / Blender workspaces).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Code,
    Paradigm,
    Agents,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Code, Preset::Paradigm, Preset::Agents];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Code => "Code",
            Preset::Paradigm => "Paradigm",
            Preset::Agents => "Agents",
        }
    }

    /// Center layout plus each dock's panels, size and open state.
    pub fn build(self, cx: &mut App) -> (DockLayout, Vec<(Side, DockLayout, Pixels, bool)>) {
        let (left, right, bottom) = (
            fixed::LEFT_DOCK_SIZE,
            fixed::RIGHT_DOCK_SIZE,
            fixed::BOTTOM_DOCK_SIZE,
        );
        match self {
            // Also the Code Editor section's default layout.
            Preset::Code => (
                tabs(&["Editor"], cx),
                vec![
                    (Side::Left, tabs(&["Explorer"], cx), left, true),
                    (Side::Right, tabs(&["Chat"], cx), right, true),
                    (Side::Bottom, tabs(&["Terminal"], cx), bottom, true),
                ],
            ),
            Preset::Paradigm => (
                tabs(&["Paradigm Canvas"], cx),
                vec![
                    (Side::Left, tabs(&["Paradigms", "Outline"], cx), left, true),
                    (Side::Right, tabs(&["Agent"], cx), right, true),
                    (Side::Bottom, tabs(&["Problems"], cx), bottom, false),
                ],
            ),
            Preset::Agents => (
                DockLayout::h_split()
                    .child(tabs(&["Agent Chat 1"], cx), None)
                    .child(tabs(&["Agent Chat 2"], cx), None),
                vec![
                    (Side::Left, tabs(&["Files"], cx), left, false),
                    (Side::Right, tabs(&["Diff Review"], cx), right, true),
                    (
                        Side::Bottom,
                        tabs(&["Agent Logs", "Terminal"], cx),
                        bottom,
                        true,
                    ),
                ],
            ),
        }
    }
}

fn tabs(titles: &[&str], cx: &mut App) -> DockLayout {
    titles.iter().fold(DockLayout::tabs(), |layout, title| {
        // Real panels by title; everything else is still a placeholder.
        let panel = match *title {
            "Explorer" => panel_handle(ExplorerPanel::new(cx)),
            "Editor" => panel_handle(EditorPanel::new(cx)),
            "Terminal" => panel_handle(TerminalPanel::new(None, None, None, cx)),
            t if crate::chat_panel::is_chat_title(t) => {
                panel_handle(crate::chat_panel::ChatPanel::new(t.to_string(), cx))
            }
            _ => panel_handle(TestPanel::new(*title, cx)),
        };
        layout.panel_view(panel, cx)
    })
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedLayout {
    pub name: String,
    pub dock: DockAreaState,
    pub auto_hide: Vec<Side>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutStore {
    pub auto_hide: Vec<Side>,
    pub saved: Vec<SavedLayout>,
}

impl LayoutStore {
    pub fn load() -> Self {
        let Ok(text) = std::fs::read_to_string(STORE_FILE) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_else(|err| {
            eprintln!("invalid {STORE_FILE}: {err}; starting empty");
            Self::default()
        })
    }

    pub fn save(&self) {
        let result = serde_json::to_string_pretty(self)
            .map_err(|err| err.to_string())
            .and_then(|json| {
                std::fs::create_dir_all(STORE_DIR)
                    .and_then(|_| std::fs::write(STORE_FILE, json))
                    .map_err(|err| err.to_string())
            });
        if let Err(err) = result {
            eprintln!("failed to save {STORE_FILE}: {err}");
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.saved
            .iter()
            .map(|layout| layout.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    // Named imports only: the `gpui_kit::*` glob would shadow the test macro.
    use super::{DockAreaState, LayoutStore, SavedLayout, Side};

    #[test]
    fn layout_store_round_trips() {
        let store = LayoutStore {
            auto_hide: vec![Side::Bottom],
            saved: vec![SavedLayout {
                name: "Review".into(),
                dock: DockAreaState::default(),
                auto_hide: vec![Side::Left, Side::Right],
            }],
        };
        let json = serde_json::to_string(&store).unwrap();
        assert!(json.contains(r#""auto_hide":["bottom"]"#));
        let back: LayoutStore = serde_json::from_str(&json).unwrap();
        assert_eq!(back.names(), ["Review"]);
        assert_eq!(back.saved[0].auto_hide, [Side::Left, Side::Right]);
        assert_eq!(back.auto_hide, [Side::Bottom]);
    }

    #[test]
    fn missing_fields_default() {
        let store: LayoutStore = serde_json::from_str("{}").unwrap();
        assert!(store.saved.is_empty() && store.auto_hide.is_empty());
    }
}
