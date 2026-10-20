//! Layout tier T2 data: built-in task presets and the user's named layouts.
//! Named layouts and auto-hide flags are per workflow, kept in the workflow
//! `state.json` (`workflows.md`); the old global `layouts.json` is read once
//! as the Editor's.

use carbon_core::workflow::{Region, VIEW_KIND, WorkflowDef, WorkflowId};
use gpui_kit::component::dock::{DockAreaState, DockLayout, DockPlacement, panel_handle};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

use crate::editor_panel::EditorPanel;
use crate::explorer_panel::ExplorerPanel;
use crate::style::fixed;
use crate::terminal_panel::TerminalPanel;
use crate::test_panel::TestPanel;
use crate::view_panel::ViewPanel;

/// Named layouts from before they were per workflow; read once, never written.
const LEGACY_STORE_FILE: &str = concat!(
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

/// Built-in task layouts (Eclipse perspectives / Blender workspaces). The
/// Paradigm and Agents presets became workflows (2026-10-20, user).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Code,
}

impl Preset {
    pub const ALL: [Preset; 1] = [Preset::Code];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Code => "Code",
        }
    }

    /// Center layout plus each dock's panels, size and open state.
    pub fn build(self, cx: &mut App) -> (DockLayout, Vec<(Side, DockLayout, Pixels, bool)>) {
        match self {
            // Same as the Editor workflow's default layout (tested below).
            Preset::Code => LayoutSpec::code().build(cx),
        }
    }
}

/// What a layout slot builds; workflow manifests name kinds
/// (`workflows::PANEL_KINDS`).
#[derive(Clone, Debug, PartialEq)]
pub enum PanelKind {
    Explorer,
    Editor,
    Terminal,
    Chat,
    /// No content; shows its title.
    Placeholder(SharedString),
    /// A declarative view of a workflow: tab title, workflow, view name.
    View(SharedString, WorkflowId, String),
    /// A web page (experimental, `webview` feature): tab title, and the
    /// package page (`None`: the spike's test page).
    #[cfg(feature = "webview")]
    Webview(SharedString, Option<crate::webview_panel::PageSource>),
}

impl PanelKind {
    /// A manifest kind with its rule's title. A kind this build cannot make
    /// (from a newer Carbon or a missing extension) becomes a placeholder
    /// saying so; the rest of the layout still loads.
    pub fn from_manifest(kind: &str, title: Option<&str>) -> Self {
        match kind {
            "explorer" => PanelKind::Explorer,
            "editor" => PanelKind::Editor,
            "terminal" => PanelKind::Terminal,
            "chat" => PanelKind::Chat,
            "placeholder" => PanelKind::Placeholder(title.unwrap_or(kind).to_string().into()),
            #[cfg(feature = "webview")]
            "webview" => PanelKind::Webview(title.unwrap_or("Web").to_string().into(), None),
            _ => PanelKind::Placeholder(format!("Panel `{kind}` is not available").into()),
        }
    }

    fn add_to(&self, layout: DockLayout, cx: &mut App) -> DockLayout {
        let panel = match self {
            PanelKind::Explorer => panel_handle(ExplorerPanel::new(cx)),
            PanelKind::Editor => panel_handle(EditorPanel::new(cx)),
            PanelKind::Terminal => panel_handle(TerminalPanel::new(None, None, None, cx)),
            PanelKind::Chat => panel_handle(crate::chat_panel::ChatPanel::new("Chat", cx)),
            PanelKind::Placeholder(title) => panel_handle(TestPanel::new(title.clone(), cx)),
            PanelKind::View(title, workflow, view) => panel_handle(ViewPanel::new(
                title.clone(),
                workflow.clone(),
                view.clone(),
                cx,
            )),
            #[cfg(feature = "webview")]
            PanelKind::Webview(title, source) => panel_handle(
                crate::webview_panel::WebviewPanel::new(title.clone(), source.clone(), cx),
            ),
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
                    let rule = def.panels.get(name);
                    let title = rule.and_then(|rule| rule.title.as_deref());
                    let kind = PanelKind::from_manifest(def.panel_kind(name), title);
                    if def.panel_kind(name) == VIEW_KIND
                        && let Some(view) = rule.and_then(|rule| rule.view.clone())
                    {
                        let title = title.unwrap_or(name).to_string().into();
                        return PanelKind::View(title, def.id.clone(), view);
                    }
                    #[cfg(feature = "webview")]
                    if let PanelKind::Webview(title, _) = &kind
                        && let Some(page) = rule.and_then(|rule| rule.page.clone())
                    {
                        let workflow = def.id.clone();
                        let source = crate::webview_panel::PageSource { workflow, page };
                        return PanelKind::Webview(title.clone(), Some(source));
                    }
                    kind
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
    /// The old global `layouts.json` (it becomes the Editor's set).
    pub fn load_legacy() -> Self {
        let Ok(text) = std::fs::read_to_string(LEGACY_STORE_FILE) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_else(|err| {
            eprintln!("invalid {LEGACY_STORE_FILE}: {err}; starting empty");
            Self::default()
        })
    }

    /// From its `state.json` entry; a broken entry starts empty.
    pub fn from_value(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_else(|err| {
            eprintln!("invalid named layouts: {err}; starting empty");
            Self::default()
        })
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
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
        let registry = registry();
        let editor = registry.get(&WorkflowId::editor()).unwrap();
        assert_eq!(LayoutSpec::of_workflow(editor), LayoutSpec::code());
    }

    #[test]
    fn placeholder_workflows_have_one_panel_and_closed_docks() {
        let registry = registry();
        let def = registry
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
    fn unknown_kind_becomes_a_not_available_placeholder() {
        assert_eq!(
            PanelKind::from_manifest("spreadsheet", Some("Sheet")),
            PanelKind::Placeholder("Panel `spreadsheet` is not available".into())
        );
        assert_eq!(
            PanelKind::from_manifest("placeholder", Some("Canvas")),
            PanelKind::Placeholder("Canvas".into())
        );
    }

    #[test]
    fn view_panels_name_their_workflow_and_view() {
        let (def, _) = carbon_core::workflow::WorkflowDef::parse(
            r#"
id = "test.views"
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"
name = "Views"
[layout.center]
panels = ["home"]
[panels]
home = { kind = "view", view = "start" }
[[views.start]]
type = "heading"
text = "Hi"
"#,
        )
        .unwrap();
        assert_eq!(
            LayoutSpec::of_workflow(&def).center,
            [PanelKind::View(
                "home".into(),
                WorkflowId::new("test.views"),
                "start".into()
            )]
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

    /// Named layouts travel through `state.json` as JSON values.
    #[test]
    fn layout_store_round_trips_as_a_state_value() {
        let store = LayoutStore {
            auto_hide: vec![Side::Left],
            saved: vec![SavedLayout {
                name: "Wide".into(),
                dock: DockAreaState::default(),
                auto_hide: Vec::new(),
            }],
        };
        let back = LayoutStore::from_value(&store.to_value());
        assert_eq!(back.names(), ["Wide"]);
        assert_eq!(back.auto_hide, [Side::Left]);
        let broken = LayoutStore::from_value(&serde_json::json!({"saved": 3}));
        assert!(broken.saved.is_empty(), "a broken entry starts empty");
    }

    #[test]
    fn missing_fields_default() {
        let store: LayoutStore = serde_json::from_str("{}").unwrap();
        assert!(store.saved.is_empty() && store.auto_hide.is_empty());
    }
}
