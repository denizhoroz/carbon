//! Layout tier T2 data: built-in task presets and the user's named layouts.
//! Named layouts and auto-hide flags live in `layouts.json` beside the current layout.

use carbon_core::workflow::{Region, WorkflowDef};
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
            // Same as the Editor workflow's default layout (tested below).
            Preset::Code => LayoutSpec::code().build(cx),
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

/// What a layout slot builds. Workflow manifests name kinds
/// (`workflows::PANEL_KINDS`); presets below still name titles (`tabs`).
#[derive(Clone, Debug, PartialEq)]
pub enum PanelKind {
    Explorer,
    Editor,
    Terminal,
    Chat,
    /// No content; shows its title.
    Placeholder(SharedString),
}

impl PanelKind {
    /// A manifest kind with its rule's title. A kind this build cannot make
    /// (validation should have rejected it) becomes a placeholder.
    pub fn from_manifest(kind: &str, title: Option<&str>) -> Self {
        match kind {
            "explorer" => PanelKind::Explorer,
            "editor" => PanelKind::Editor,
            "terminal" => PanelKind::Terminal,
            "chat" => PanelKind::Chat,
            _ => PanelKind::Placeholder(title.unwrap_or(kind).to_string().into()),
        }
    }

    fn add_to(&self, layout: DockLayout, cx: &mut App) -> DockLayout {
        let panel = match self {
            PanelKind::Explorer => panel_handle(ExplorerPanel::new(cx)),
            PanelKind::Editor => panel_handle(EditorPanel::new(cx)),
            PanelKind::Terminal => panel_handle(TerminalPanel::new(None, None, None, cx)),
            PanelKind::Chat => panel_handle(crate::chat_panel::ChatPanel::new("Chat", cx)),
            PanelKind::Placeholder(title) => panel_handle(TestPanel::new(title.clone(), cx)),
        };
        layout.panel_view(panel, cx)
    }
}

/// One dock of a [`LayoutSpec`].
#[derive(Clone, Debug, PartialEq)]
pub struct DockSpec {
    pub side: Side,
    pub panels: Vec<PanelKind>,
    pub size: Pixels,
    pub open: bool,
}

/// A default layout as data: the center's tabs and all three docks (an
/// unused dock is empty and closed, so nothing stays from the previous layout).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutSpec {
    pub center: Vec<PanelKind>,
    pub docks: Vec<DockSpec>,
}

impl LayoutSpec {
    /// The Code preset: Explorer left, editors center, Chat right, Terminal below.
    pub fn code() -> Self {
        let dock = |side: Side, panel| DockSpec {
            side,
            panels: vec![panel],
            size: side.default_size(),
            open: true,
        };
        Self {
            center: vec![PanelKind::Editor],
            docks: vec![
                dock(Side::Left, PanelKind::Explorer),
                dock(Side::Right, PanelKind::Chat),
                dock(Side::Bottom, PanelKind::Terminal),
            ],
        }
    }

    /// A workflow's default layout from its manifest.
    pub fn of_workflow(def: &WorkflowDef) -> Self {
        let kinds = |region: &Region| {
            region
                .panels
                .iter()
                .map(|name| {
                    let title = def.panels.get(name).and_then(|rule| rule.title.as_deref());
                    PanelKind::from_manifest(def.panel_kind(name), title)
                })
                .collect::<Vec<_>>()
        };
        let docks = Side::ALL
            .into_iter()
            .map(|side| {
                let region = match side {
                    Side::Left => &def.layout.left,
                    Side::Right => &def.layout.right,
                    Side::Bottom => &def.layout.bottom,
                };
                match region {
                    Some(region) => DockSpec {
                        side,
                        panels: kinds(region),
                        size: region.size.map(px).unwrap_or(side.default_size()),
                        open: region.open,
                    },
                    None => DockSpec {
                        side,
                        panels: Vec::new(),
                        size: side.default_size(),
                        open: false,
                    },
                }
            })
            .collect();
        Self {
            center: def.layout.center.as_ref().map(kinds).unwrap_or_default(),
            docks,
        }
    }

    /// Center layout plus each dock's panels, size and open state.
    pub fn build(&self, cx: &mut App) -> (DockLayout, Vec<(Side, DockLayout, Pixels, bool)>) {
        let tabs = |panels: &[PanelKind], cx: &mut App| {
            panels
                .iter()
                .fold(DockLayout::tabs(), |layout, kind| kind.add_to(layout, cx))
        };
        let center = tabs(&self.center, cx);
        let docks = self
            .docks
            .iter()
            .map(|dock| (dock.side, tabs(&dock.panels, cx), dock.size, dock.open))
            .collect();
        (center, docks)
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
    use carbon_core::workflow::WorkflowId;

    use super::{DockAreaState, LayoutSpec, LayoutStore, PanelKind, SavedLayout, Side};
    use crate::workflows::registry;

    /// The Editor workflow keeps the layout the Code preset always had.
    #[test]
    fn editor_workflow_layout_is_the_code_preset() {
        let editor = registry().get(&WorkflowId::editor()).unwrap();
        assert_eq!(LayoutSpec::of_workflow(editor), LayoutSpec::code());
    }

    #[test]
    fn placeholder_workflows_have_one_panel_and_closed_docks() {
        let def = registry()
            .get(&WorkflowId::new("carbon.paradigm-editor"))
            .unwrap();
        let spec = LayoutSpec::of_workflow(def);
        assert_eq!(
            spec.center,
            [PanelKind::Placeholder("Paradigm Editor".into())]
        );
        assert_eq!(spec.docks.len(), 3);
        assert!(
            spec.docks
                .iter()
                .all(|dock| dock.panels.is_empty() && !dock.open)
        );
    }

    #[test]
    fn every_builtin_loads() {
        assert!(registry().rejected().is_empty());
        assert_eq!(registry().workflows().len(), 3);
    }

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
