//! Main dock area. Tier T1 (kit `DockArea`: drag, snap, split, resize, close, zoom,
//! persist) plus T2: named layouts, task presets, keyboard move/resize, auto-hide docks.
//! Files live in the build dir for now; they move to the OS user dir later.

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::Placement;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{
    BasePanel, BasePanelView, DockArea, DockAreaState, DockEvent, DockLayout, DockPlacement,
    DockSkin, InsertTarget, NodeId, PaneRef, PaneTree, PanelId,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt, h_flex};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

use carbon_core::BufferId;
use gpui_kit::component::dock::panel_handle;

use crate::chat_panel::ChatPanel;
use crate::editor_panel::EditorPanel;
use crate::explorer_panel::ExplorerPanel;
use crate::extension_panel::{ADD_PREFIX, ExtensionPanel};
use crate::layouts::{LayoutStore, MAX_SAVED, Preset, SavedLayout, Side};
use crate::menus::{Direction, MenuState};
use crate::style::fixed;
use crate::terminal_panel::TerminalPanel;
use crate::test_panel::TestPanel;
use carbon_core::terminal::shells;

/// Saved-layout schema version; bump when old layouts can no longer load.
/// 2: Explorer is a real panel (v1 layouts hold a placeholder "Explorer").
const LAYOUT_VERSION: usize = 2;
const LAYOUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/carbon-gui");
const LAYOUT_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/carbon-gui/layout.json"
);
const SAVE_DELAY: Duration = Duration::from_millis(500);
const CENTER_AND_DOCKS: [DockPlacement; 4] = [
    DockPlacement::Center,
    DockPlacement::Left,
    DockPlacement::Right,
    DockPlacement::Bottom,
];

/// The Explorer section: the left dock (or the right one, from the
/// Explorer's "..." menu: `Workbench::toggle_section_side`), holding the Explorer. Fixed
/// (`DockArea::set_dock_fixed`, kit patches): its tabs rearrange and split
/// inside it, but none closes or leaves and no other panel comes in. It still
/// collapses (Ctrl+B, View > Toggle Left Dock, dragging its edge shut). Tabs
/// beside the Explorer come from extensions (`explorer_section` panels):
/// added when the extension is active, removed when it is not.
fn section_placement(right: bool) -> DockPlacement {
    if right {
        DockPlacement::Right
    } else {
        DockPlacement::Left
    }
}

/// Whether the Explorer section is on the right (the Explorer's "..." menu).
pub fn section_on_right(cx: &App) -> bool {
    current(cx).is_some_and(|workbench| workbench.read(cx).store.explorer_right)
}

/// A shared callback run with the window (dialog buttons and Enter share one).
pub type WindowCallback = Rc<dyn Fn(&mut Window, &mut App)>;

/// The window's workbench, for panels that open or close tabs. One window
/// for now, like `CurrentCore`.
pub struct CurrentWorkbench(pub WeakEntity<Workbench>);

impl Global for CurrentWorkbench {}

fn current(cx: &App) -> Option<Entity<Workbench>> {
    cx.try_global::<CurrentWorkbench>()?.0.upgrade()
}

/// Shows buffer `id` in an editor tab: its own tab if it has one, else an
/// empty editor tab, else a new tab in the center. Focuses the editor.
pub fn open_buffer(id: BufferId, window: &mut Window, cx: &mut App) {
    if let Some(workbench) = current(cx) {
        workbench.update(cx, |workbench, cx| workbench.show_buffer(id, window, cx));
    }
}

/// Opens the file at `path` in an editor tab, like a click in the Explorer.
/// Reports a failure as a notification. A graph file (`*.flow.yaml` of an
/// active extension) opens in a graph panel instead.
pub fn open_file_at(path: &Path, window: &mut Window, cx: &mut App) {
    if crate::graph_panel::open_graph_file(path, window, cx) {
        return;
    }
    let core = crate::workspace_ops::current_core(cx);
    let result = core.update(cx, |core, cx| {
        let result = core.open_file(path);
        cx.notify();
        result
    });
    match result {
        Ok(id) => open_buffer(id, window, cx),
        Err(err) => window.push_notification(format!("Cannot open file: {err}"), cx),
    }
}

/// Shows the graph file `path` in its graph panel: the tab that has it, else
/// a new tab in the editor area (extension panel `key`).
pub fn open_graph(path: &Path, key: String, title: String, window: &mut Window, cx: &mut App) {
    let Some(workbench) = current(cx) else {
        return;
    };
    let area = workbench.read(cx).area.clone();
    let wanted = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut found = None;
    for placement in CENTER_AND_DOCKS {
        let Some(tree) = area.read(cx).layout(placement) else {
            continue;
        };
        for panel in tree.panels() {
            let graph = area.read(cx).panel(panel).and_then(|view| {
                view.view()
                    .downcast::<crate::graph_panel::GraphPanel>()
                    .ok()
            });
            let same = graph.is_some_and(|graph| {
                graph.read(cx).path().is_some_and(|p| {
                    dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()) == wanted
                })
            });
            if same {
                found = Some(panel);
            }
        }
    }
    let panel = match found {
        Some(panel) => panel,
        None => {
            let handle = panel_handle(crate::graph_panel::GraphPanel::new(
                key,
                title,
                Some(wanted),
                cx,
            ));
            let panel = handle.panel_id(cx);
            area.update(cx, |area, cx| {
                area.add_panel_view(handle, DockPlacement::Center, None, window, cx)
            });
            panel
        }
    };
    area.update(cx, |area, cx| area.select_panel(panel, window, cx));
    workbench.update(cx, |workbench, cx| workbench.schedule_save(cx));
}

/// The editor tab of the core's active buffer, for actions that reach the
/// app root (from a menu, or with focus outside the editors).
pub fn active_editor(cx: &App) -> Option<Entity<EditorPanel>> {
    let id = crate::workspace_ops::current_core(cx)
        .read(cx)
        .active_buffer()?
        .id();
    let workbench = current(cx)?;
    let (_, editor, _) = workbench
        .read(cx)
        .editors(cx)
        .into_iter()
        .find(|(_, editor, _)| editor.read(cx).buffer() == Some(id))?;
    Some(editor)
}

/// Closes an editor tab. The editor area's last tab is cleared to the empty
/// editor instead, so the area never goes blank. Deferred: the caller is
/// often the editor itself, which the dock then tells it was removed.
pub fn close_editor(editor: Entity<EditorPanel>, window: &mut Window, cx: &mut App) {
    let Some(workbench) = current(cx) else {
        return;
    };
    window.defer(cx, move |window, cx| {
        let area = workbench.read(cx).area.clone();
        let id = PanelId::from(editor.entity_id());
        let center: Vec<PanelId> = area
            .read(cx)
            .layout(DockPlacement::Center)
            .map(|tree| tree.panels().collect())
            .unwrap_or_default();
        if center == [id] {
            editor.update(cx, |editor, cx| editor.clear(cx));
            workbench.update(cx, |workbench, cx| workbench.schedule_save(cx));
        } else {
            area.update(cx, |area, cx| area.remove_panel(editor, window, cx));
        }
    });
}

/// Closes a panel's tab (Chat, Terminal, ...; editors use `close_editor`).
/// The center never goes blank: closing its last panel leaves an empty
/// editor. Panels of the Explorer section never close. Deferred: the caller
/// is inside the panel's update.
pub fn close_panel<P: BasePanel>(panel: Entity<P>, window: &mut Window, cx: &mut App) {
    let Some(workbench) = current(cx) else {
        return;
    };
    window.defer(cx, move |window, cx| {
        let area = workbench.read(cx).area.clone();
        let id = PanelId::from(panel.entity_id());
        let placement = CENTER_AND_DOCKS.into_iter().find(|&placement| {
            area.read(cx)
                .layout(placement)
                .is_some_and(|tree| tree.panels().any(|panel| panel == id))
        });
        let only = placement.is_some_and(|placement| {
            area.read(cx)
                .layout(placement)
                .is_some_and(|tree| tree.panels().eq([id]))
        });
        if placement == Some(workbench.read(cx).section()) {
            return;
        }
        area.update(cx, |area, cx| {
            area.remove_panel(panel, window, cx);
            match placement {
                Some(DockPlacement::Center) if only => {
                    let editor = panel_handle(EditorPanel::new(cx));
                    area.add_panel_view(editor, DockPlacement::Center, None, window, cx);
                }
                // An emptied dock closes instead of leaving a blank strip.
                Some(placement) if only && area.is_dock_open(placement) => {
                    area.toggle_dock(placement, window, cx)
                }
                _ => {}
            }
        });
        workbench.update(cx, |workbench, cx| workbench.schedule_save(cx));
    });
}

/// Saved layouts or auto-hide flags changed; the owner rebuilds the menus.
pub struct MenusChanged;

/// The layout Carbon starts with (and resets to).
const DEFAULT_PRESET: Preset = Preset::Code;

/// `layout.json`: the current layout. Files from before sections were removed
/// (2026-10-22) hold `{ active, sections: { code: ... } }`; the Code Editor's
/// layout is read from there once and the file is rewritten in this shape.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct LayoutFile {
    dock: Option<DockAreaState>,
    #[serde(skip_serializing)]
    sections: HashMap<String, DockAreaState>,
}

impl LayoutFile {
    fn into_dock(mut self) -> Option<DockAreaState> {
        self.dock.or_else(|| self.sections.remove("code"))
    }
}

pub struct Workbench {
    area: Entity<DockArea>,
    store: LayoutStore,
    _layout_sub: Subscription,
    _save_task: Task<()>,
}

impl EventEmitter<MenusChanged> for Workbench {}

impl Workbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (area, skin) = DockSkin::dock_area("carbon-main", Some(LAYOUT_VERSION), window, cx);
        let store = LayoutStore::load();
        let section = section_placement(store.explorer_right);
        skin.set_close_button_visible(true, cx);
        // No dock collapse buttons in tab bars (user): docks open and close by
        // dragging their edge, the View menu and Ctrl+B / Ctrl+Alt+B / Ctrl+J.
        skin.set_toggle_button_visible(false, cx);
        area.update(cx, |area, cx| {
            area.set_dock_fixed(section, true, window, cx)
        });

        let _layout_sub = cx.subscribe_in(&area, window, |this, _, event, window, cx| {
            if let DockEvent::LayoutChanged = event {
                this.repair_layout(window, cx);
                this.schedule_save(cx);
            }
        });

        let mut this = Self {
            area,
            store,
            _layout_sub,
            _save_task: Task::ready(()),
        };
        if !this.restore(window, cx) {
            this.apply_preset(DEFAULT_PRESET, window, cx);
        }
        this.keep_explorer_section(window, cx);
        this.close_empty_docks(window, cx);
        // Auto-hide docks start hidden, like JetBrains unpinned tool windows.
        for side in this.store.auto_hide.clone() {
            this.set_dock_open(side.placement(), false, window, cx);
        }
        this
    }

    /// Every editor tab, center first, with its dock placement.
    fn editors(&self, cx: &App) -> Vec<(PanelId, Entity<EditorPanel>, DockPlacement)> {
        let area = self.area.read(cx);
        let mut editors = Vec::new();
        for placement in CENTER_AND_DOCKS {
            let Some(tree) = area.layout(placement) else {
                continue;
            };
            for panel in tree.panels() {
                if let Some(editor) = area
                    .panel(panel)
                    .and_then(|view| view.view().downcast::<EditorPanel>().ok())
                {
                    editors.push((panel, editor, placement));
                }
            }
        }
        editors
    }

    fn show_buffer(&mut self, id: BufferId, window: &mut Window, cx: &mut Context<Self>) {
        let mut own = None;
        let mut empty = None;
        for (panel, editor, placement) in self.editors(cx) {
            match editor.read(cx).buffer() {
                Some(buffer) if buffer == id => own = Some((panel, editor)),
                None if empty.is_none() && placement == DockPlacement::Center => {
                    empty = Some((panel, editor))
                }
                _ => {}
            }
        }
        let (panel, editor) = match (own, empty) {
            (Some(own), _) => own,
            (None, Some((panel, editor))) => {
                editor.update(cx, |editor, cx| editor.set_buffer(id, window, cx));
                (panel, editor)
            }
            (None, None) => {
                let editor = EditorPanel::for_buffer(id, window, cx);
                let handle = panel_handle(editor.clone());
                let panel = handle.panel_id(cx);
                self.area.update(cx, |area, cx| {
                    area.add_panel_view(handle, DockPlacement::Center, None, window, cx)
                });
                (panel, editor)
            }
        };
        self.area
            .update(cx, |area, cx| area.select_panel(panel, window, cx));
        // Deferred: a menu that ran this (File > New File) gives focus back
        // to where it was when it closes, after its action.
        window.defer(cx, move |window, cx| {
            editor.update(cx, |editor, cx| editor.focus_editor(window, cx))
        });
        // A reused tab changes no layout node, but its saved file does.
        self.schedule_save(cx);
    }

    /// View > Add Panel, or a panel's "New ..." item.
    /// View > Add Panel, a panel's "New ..." item, or a terminal's "New
    /// <shell>" (`profile`: that shell; otherwise the default).
    pub fn add_panel(
        &mut self,
        name: &str,
        profile: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let section = self.section();
        // Every panel with its place; placeholders also with their kind.
        let mut panels: Vec<(PanelId, DockPlacement, Option<SharedString>, bool)> = Vec::new();
        {
            let area = self.area.read(cx);
            for placement in CENTER_AND_DOCKS {
                let Some(tree) = area.layout(placement) else {
                    continue;
                };
                for id in tree.panels() {
                    let Some(view) = area.panel(id).map(|view| view.view()) else {
                        continue;
                    };
                    let kind = view
                        .clone()
                        .downcast::<TestPanel>()
                        .ok()
                        .map(|panel| panel.read(cx).kind())
                        .or_else(|| {
                            view.clone()
                                .downcast::<TerminalPanel>()
                                .is_ok()
                                .then(|| "Terminal".into())
                        })
                        .or_else(|| {
                            view.clone()
                                .downcast::<ChatPanel>()
                                .is_ok()
                                .then(|| "Chat".into())
                        })
                        .or_else(|| {
                            let panel = view.clone().downcast::<ExtensionPanel>().ok()?;
                            Some(format!("{ADD_PREFIX}{}", panel.read(cx).key()).into())
                        });
                    let explorer = view.downcast::<ExplorerPanel>().is_ok();
                    panels.push((id, placement, kind, explorer));
                }
            }
        }
        // An extension panel that may be open once: show the open one.
        let single_extension = name
            .strip_prefix(ADD_PREFIX)
            .and_then(|key| {
                crate::extensions::registry(cx)
                    .panel(key)
                    .map(|(_, p)| !p.multi)
            })
            .unwrap_or(false);
        let open_extension = panels
            .iter()
            .find(|(_, _, kind, _)| kind.as_deref() == Some(name))
            .map(|&(id, placement, ..)| (id, placement));
        let (id, placement, handle) =
            if single_extension && let Some((id, placement)) = open_extension {
                (id, placement, None)
            } else if name == "Explorer" {
                match panels.iter().find(|(.., explorer)| *explorer) {
                    // There is one Explorer: show it.
                    Some(&(id, placement, ..)) => (id, placement, None),
                    None => {
                        let handle = panel_handle(ExplorerPanel::new(cx));
                        (handle.panel_id(cx), section, Some(handle))
                    }
                }
            } else {
                // Never into the Explorer section, even beside one of its kind.
                let same: Vec<_> = panels
                    .iter()
                    .filter(|(_, placement, kind, _)| {
                        kind.as_deref() == Some(name) && *placement != section
                    })
                    .collect();
                let placement = same
                    .first()
                    .map(|(_, placement, ..)| *placement)
                    .unwrap_or_else(|| default_placement(name, cx));
                let titles: Vec<SharedString> = same
                    .iter()
                    .filter_map(|(id, ..)| {
                        let view = self.area.read(cx).panel(*id)?.view();
                        if let Ok(panel) = view.clone().downcast::<TestPanel>() {
                            return Some(panel.read(cx).title());
                        }
                        if let Ok(panel) = view.clone().downcast::<ChatPanel>() {
                            return Some(panel.read(cx).label());
                        }
                        if let Ok(panel) = view.clone().downcast::<ExtensionPanel>() {
                            return Some(panel.read(cx).title());
                        }
                        Some(view.downcast::<TerminalPanel>().ok()?.read(cx).label())
                    })
                    .collect();
                let handle = if name == "Terminal" {
                    let profile = profile
                        .filter(|name| !name.is_empty())
                        .and_then(shells::profile_named)
                        .or_else(shells::default_profile);
                    let base = profile
                        .as_ref()
                        .map_or_else(|| "Terminal".to_string(), |profile| profile.name.clone());
                    let label = next_title(&base, &titles).into();
                    // New terminals start where the last one was.
                    let cwd = crate::terminal_panel::active_terminal_cwd(cx);
                    panel_handle(TerminalPanel::new(profile, Some(label), cwd, cx))
                } else if name == "Chat" {
                    panel_handle(ChatPanel::new(next_title(name, &titles), cx))
                } else if let Some(key) = name.strip_prefix(ADD_PREFIX) {
                    let title = crate::extensions::registry(cx)
                        .panel(key)
                        .map(|(_, panel)| panel.title.clone())
                        .unwrap_or_else(|| key.to_string());
                    let title = next_title(&title, &titles);
                    extension_panel_handle(key, title, cx)
                } else {
                    panel_handle(TestPanel::new(next_title(name, &titles), cx))
                };
                (handle.panel_id(cx), placement, Some(handle))
            };
        self.area.update(cx, |area, cx| {
            if let Some(handle) = handle {
                let size = Side::of(placement).map(Side::default_size);
                area.add_panel_view(handle, placement, size, window, cx);
            }
            if placement != DockPlacement::Center && !area.is_dock_open(placement) {
                area.toggle_dock(placement, window, cx);
            }
            area.select_panel(id, window, cx);
        });
        if let Some(view) = self.area.read(cx).panel(id).cloned() {
            // Deferred: the menu that ran this gives focus back when it closes.
            window.defer(cx, move |window, cx| {
                view.focus_handle(cx).focus(window, cx)
            });
        }
        self.schedule_save(cx);
    }

    pub fn menu_state(&self) -> MenuState {
        MenuState {
            has_workspace: false,
            saved: self.store.names(),
            auto_hide: self.store.auto_hide.clone(),
            ..MenuState::default()
        }
    }

    pub fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_preset(DEFAULT_PRESET, window, cx);
    }

    // ---- docks -----------------------------------------------------------

    pub fn toggle_dock(&mut self, side: Side, window: &mut Window, cx: &mut Context<Self>) {
        let placement = side.placement();
        self.area
            .update(cx, |area, cx| area.toggle_dock(placement, window, cx));
        // An auto-hide dock takes focus when shown, so keyboard users land in it.
        if self.store.auto_hide.contains(&side)
            && self.area.read(cx).is_dock_open(placement)
            && let Some(panel) = self.first_panel(placement, cx)
        {
            panel.focus_handle(cx).focus(window, cx);
        }
    }

    /// After any change (a drag between regions, a keyboard move): the center
    /// never goes blank, an empty editor takes the place of a panel moved out;
    /// a dock whose last panel left closes; the Explorer section holds the
    /// Explorer.
    fn repair_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.keep_explorer_section(window, cx);
        let center_empty = self
            .area
            .read(cx)
            .layout(DockPlacement::Center)
            .is_none_or(|tree| tree.panels().next().is_none());
        if center_empty {
            let editor = panel_handle(EditorPanel::new(cx));
            self.area.update(cx, |area, cx| {
                area.add_panel_view(editor, DockPlacement::Center, None, window, cx)
            });
        }
        self.close_empty_docks(window, cx);
    }

    /// Extensions changed: their Explorer-section tabs come and go.
    pub fn extensions_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.keep_explorer_section(window, cx);
        self.schedule_save(cx);
        cx.notify();
    }

    /// The Explorer section holds the Explorer, first, and the tabs active
    /// extensions put beside it. A layout saved before the section was fixed
    /// is put right: a missing Explorer comes back, one elsewhere moves in
    /// (extra ones go), and other panels in the section move to where their
    /// kind goes.
    fn keep_explorer_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let section = self.section();
        let mut explorers = Vec::new();
        let mut strays = Vec::new();
        // Extension tabs that belong in the section, and those present.
        let registry = crate::extensions::registry(cx);
        let wanted: Vec<String> = registry
            .panels()
            .into_iter()
            .filter(|(_, panel)| panel.explorer_section)
            .map(|(loaded, panel)| format!("{}/{}", loaded.def.id, panel.id))
            .collect();
        let mut present: Vec<String> = Vec::new();
        let mut unwanted: Vec<Entity<ExtensionPanel>> = Vec::new();
        {
            let area = self.area.read(cx);
            for placement in CENTER_AND_DOCKS {
                let Some(tree) = area.layout(placement) else {
                    continue;
                };
                for id in tree.panels() {
                    let Some(view) = area.panel(id) else {
                        continue;
                    };
                    let name = view.panel_name(cx);
                    let extension = view.view().downcast::<ExtensionPanel>().ok();
                    let key = extension
                        .as_ref()
                        .map(|panel| panel.read(cx).key().to_string());
                    if name == crate::explorer_panel::PANEL_NAME {
                        explorers.push((id, placement, view.view()));
                    } else if let (Some(panel), Some(key)) = (extension, key.as_ref())
                        && (wanted.contains(key) || placement == section)
                    {
                        // A section tab elsewhere, or a tab whose extension
                        // no longer puts it here: it goes (and comes back
                        // where it belongs).
                        if wanted.contains(key) && placement == section {
                            present.push(key.clone());
                        } else {
                            unwanted.push(panel);
                        }
                    } else if placement == section {
                        strays.push((id, default_placement(name, cx)));
                    }
                }
            }
        }
        let explorer_in_place = explorers
            .first()
            .is_some_and(|(_, placement, _)| *placement == section);
        let missing: Vec<String> = wanted
            .iter()
            .filter(|key| !present.contains(key))
            .cloned()
            .collect();
        if explorers.len() == 1
            && explorer_in_place
            && strays.is_empty()
            && unwanted.is_empty()
            && missing.is_empty()
        {
            return;
        }

        self.area.update(cx, |area, cx| {
            // One Explorer: extra ones (from an older layout) go.
            for (_, _, view) in explorers.iter().skip(1) {
                if let Ok(extra) = view.clone().downcast::<ExplorerPanel>() {
                    area.remove_panel(extra, window, cx);
                }
            }
            if area.layout(section).is_none() {
                area.set_dock(section, DockLayout::tabs(), window, cx);
                area.set_dock_size(section, Side::Left.default_size(), window, cx);
            }
            let explorer = match explorers.first() {
                Some((id, ..)) => *id,
                None => {
                    let handle = panel_handle(ExplorerPanel::new(cx));
                    let id = handle.panel_id(cx);
                    area.add_panel_view(handle, section, None, window, cx);
                    id
                }
            };
            if !explorer_in_place && let Some(node) = area.layout(section).and_then(first_group) {
                let target = InsertTarget::Tabs {
                    node,
                    ix: Some(0),
                    activate: true,
                };
                area.move_panel(explorer, target, window, cx);
            }
            for panel in unwanted {
                area.remove_panel(panel, window, cx);
            }
            let added = !missing.is_empty();
            for key in missing {
                let title = registry
                    .panel(&key)
                    .map(|(_, panel)| panel.title.clone())
                    .unwrap_or_default();
                let handle = panel_handle(ExtensionPanel::new(key, title, cx));
                area.add_panel_view(handle, section, None, window, cx);
            }
            // A tab an extension adds does not take the Explorer's place.
            if added {
                area.select_panel(explorer, window, cx);
            }
            for (id, to) in strays {
                if let Some(target) = area.layout(to).map(insert_target) {
                    area.move_panel(id, target, window, cx);
                    if to != DockPlacement::Center && !area.is_dock_open(to) {
                        area.toggle_dock(to, window, cx);
                    }
                }
            }
        });
    }

    /// Docks whose panels were all closed stay closed (older layouts saved
    /// them open and blank).
    fn close_empty_docks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for side in Side::ALL {
            let placement = side.placement();
            let empty = self
                .area
                .read(cx)
                .layout(placement)
                .is_none_or(|tree| tree.panels().next().is_none());
            if empty {
                self.set_dock_open(placement, false, window, cx);
            }
        }
    }

    fn set_dock_open(
        &mut self,
        placement: DockPlacement,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.area.update(cx, |area, cx| {
            if area.has_dock(placement) && area.is_dock_open(placement) != open {
                area.toggle_dock(placement, window, cx);
            }
        });
    }

    pub fn toggle_auto_hide(&mut self, side: Side, cx: &mut Context<Self>) {
        if let Some(ix) = self.store.auto_hide.iter().position(|s| *s == side) {
            self.store.auto_hide.remove(ix);
        } else {
            self.store.auto_hide.push(side);
        }
        self.store.save();
        cx.emit(MenusChanged);
    }

    /// Closes auto-hide docks when a click lands in the dock area but outside them.
    /// Called after the click is dispatched, with its window position.
    pub fn hide_auto_docks_outside(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for side in self.store.auto_hide.clone() {
            let placement = side.placement();
            let area = self.area.read(cx);
            if !area.is_dock_open(placement) {
                continue;
            }
            let bounds = area.bounds();
            let size = area.dock_size(placement).unwrap_or_default();
            let inside = match side {
                Side::Left => position.x < bounds.left() + size,
                Side::Right => position.x > bounds.right() - size,
                Side::Bottom => position.y > bounds.bottom() - size,
            };
            if bounds.contains(&position) && !inside {
                self.set_dock_open(placement, false, window, cx);
            }
        }
    }

    // ---- keyboard move / resize ------------------------------------------

    /// The panel holding focus and where it sits.
    fn focused_panel(&self, window: &Window, cx: &App) -> Option<(PanelId, DockPlacement)> {
        let area = self.area.read(cx);
        CENTER_AND_DOCKS.into_iter().find_map(|placement| {
            let tree = area.layout(placement)?;
            tree.panels()
                .find(|id| {
                    area.panel(*id)
                        .is_some_and(|panel| panel.focus_handle(cx).contains_focused(window, cx))
                })
                .map(|id| (id, placement))
        })
    }

    fn first_panel(&self, placement: DockPlacement, cx: &App) -> Option<Arc<dyn BasePanelView>> {
        let area = self.area.read(cx);
        let id = area.layout(placement)?.panels().next()?;
        area.panel(id).cloned()
    }

    pub fn move_panel(&mut self, side: Option<Side>, window: &mut Window, cx: &mut Context<Self>) {
        let Some((panel, from)) = self.focused_panel(window, cx) else {
            return;
        };
        let section = self.section();
        let to = side.map_or(DockPlacement::Center, Side::placement);
        // The Explorer section keeps its panels and takes no others.
        if from == to || from == section || to == section {
            return;
        }
        let Some(target) = self.area.read(cx).layout(to).map(insert_target) else {
            return;
        };
        self.area.update(cx, |area, cx| {
            area.move_panel(panel, target, window, cx);
            if to != DockPlacement::Center && !area.is_dock_open(to) {
                area.toggle_dock(to, window, cx);
            }
        });
        if let Some(view) = self.area.read(cx).panel(panel).cloned() {
            view.focus_handle(cx).focus(window, cx);
        }
    }

    /// Moves the focused panel into the editor area as a new group on one side
    /// of everything there (Right: a terminal next to the code).
    pub fn move_beside_editor(
        &mut self,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((panel, from)) = self.focused_panel(window, cx) else {
            return;
        };
        let section = self.section();
        let Some(center) = self.area.read(cx).layout(DockPlacement::Center) else {
            return;
        };
        // Alone in the editor area there is nothing to sit beside; the
        // Explorer section keeps its panels.
        if from == section || (from == DockPlacement::Center && center.panels().eq([panel])) {
            return;
        }
        let target = InsertTarget::Split {
            node: center.root().id(),
            placement: match direction {
                Direction::Left => Placement::Left,
                Direction::Right => Placement::Right,
                Direction::Up => Placement::Top,
                Direction::Down => Placement::Bottom,
            },
            size: None,
        };
        self.area
            .update(cx, |area, cx| area.move_panel(panel, target, window, cx));
        if let Some(view) = self.area.read(cx).panel(panel).cloned() {
            view.focus_handle(cx).focus(window, cx);
        }
    }

    pub fn resize_dock(
        &mut self,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(side) = self
            .focused_panel(window, cx)
            .and_then(|(_, p)| Side::of(p))
        else {
            return;
        };
        let step = fixed::DOCK_RESIZE_STEP;
        let delta = match (side, direction) {
            (Side::Left, Direction::Right)
            | (Side::Right, Direction::Left)
            | (Side::Bottom, Direction::Up) => step,
            (Side::Left, Direction::Left)
            | (Side::Right, Direction::Right)
            | (Side::Bottom, Direction::Down) => -step,
            _ => return,
        };
        let placement = side.placement();
        self.area.update(cx, |area, cx| {
            let size = area.dock_size(placement).unwrap_or_default();
            let new_size = (size + delta).max(fixed::DOCK_MIN_SIZE);
            area.set_dock_size(placement, new_size, window, cx);
        });
    }

    // ---- presets and named layouts ---------------------------------------

    pub fn apply_preset(&mut self, preset: Preset, window: &mut Window, cx: &mut Context<Self>) {
        let (center, docks) = preset.build(cx);
        self.area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
            for (side, layout, size, open) in docks {
                let placement = side.placement();
                area.set_dock(placement, layout, window, cx);
                area.set_dock_size(placement, size, window, cx);
                if area.is_dock_open(placement) != open {
                    area.toggle_dock(placement, window, cx);
                }
            }
        });
        // Presets put the Explorer on the left: mirror them for a section on
        // the right (the fixed flag stays with the section).
        if self.store.explorer_right {
            self.area.update(cx, |area, cx| {
                area.swap_docks(DockPlacement::Left, DockPlacement::Right, window, cx);
                area.set_dock_fixed(DockPlacement::Left, false, window, cx);
                area.set_dock_fixed(DockPlacement::Right, true, window, cx);
            });
        }
    }

    /// Where the Explorer section is.
    fn section(&self) -> DockPlacement {
        section_placement(self.store.explorer_right)
    }

    /// Moves the Explorer section to the other side: the left and right docks
    /// trade places with everything in them (no panel is closed), and an
    /// auto-hide setting goes with its dock.
    pub fn toggle_section_side(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.area.update(cx, |area, cx| {
            area.swap_docks(DockPlacement::Left, DockPlacement::Right, window, cx)
        });
        self.store.explorer_right = !self.store.explorer_right;
        for side in &mut self.store.auto_hide {
            *side = match side {
                Side::Left => Side::Right,
                Side::Right => Side::Left,
                Side::Bottom => Side::Bottom,
            };
        }
        self.store.save();
        self.schedule_save(cx);
        cx.notify();
    }

    pub fn save_layout_as(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let layout = SavedLayout {
            name: name.clone(),
            dock: self.area.read(cx).dump(cx),
            auto_hide: self.store.auto_hide.clone(),
        };
        if let Some(existing) = self.store.saved.iter_mut().find(|l| l.name == name) {
            *existing = layout;
        } else if self.store.saved.len() >= MAX_SAVED {
            window.push_notification(
                format!("At most {MAX_SAVED} saved layouts; delete one first (View > Layout > Delete Layout)."),
                cx,
            );
            return;
        } else {
            self.store.saved.push(layout);
        }
        self.store.save();
        cx.emit(MenusChanged);
        window.push_notification(format!("Layout \"{name}\" saved."), cx);
    }

    pub fn apply_layout(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.store.saved.get(index).cloned() else {
            return;
        };
        if layout.dock.version != Some(LAYOUT_VERSION) {
            window.push_notification(
                format!(
                    "\"{}\" was saved by an older Carbon layout format; save it again.",
                    layout.name
                ),
                cx,
            );
            return;
        }
        let result = self
            .area
            .update(cx, |area, cx| area.load(layout.dock, window, cx));
        if let Err(err) = result {
            window.push_notification(format!("Could not apply \"{}\": {err}", layout.name), cx);
            return;
        }
        self.store.auto_hide = layout.auto_hide;
        self.store.save();
        self.keep_explorer_section(window, cx);
        self.close_empty_docks(window, cx);
        cx.emit(MenusChanged);
    }

    pub fn delete_layout(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.store.saved.len() {
            self.store.saved.remove(index);
            self.store.save();
            cx.emit(MenusChanged);
        }
    }

    /// Asks for a name, then saves the current layout under it.
    pub fn open_save_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Layout name"));
        let this = cx.weak_entity();

        let submit: WindowCallback = {
            let input = input.clone();
            Rc::new(move |window, cx| {
                let name = input.read(cx).value().trim().to_string();
                if name.is_empty() {
                    return;
                }
                window.close_dialog(cx);
                _ = this.update(cx, |this, cx| this.save_layout_as(name, window, cx));
            })
        };

        let on_enter = submit.clone();
        cx.subscribe_in(
            &input,
            window,
            move |_, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    on_enter(window, cx);
                }
            },
        )
        .detach();

        let dialog_input = input.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let on_save = submit.clone();
            dialog
                .title("Save Layout")
                .child(Input::new(&dialog_input))
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("save")
                                .primary()
                                .label("Save")
                                .on_click(move |_, window, cx| on_save(window, cx)),
                        ),
                )
        });
        input.update(cx, |input, cx| input.focus(window, cx));
    }

    // ---- persistence of the current layout --------------------------------

    /// Replacing the task cancels the pending save, so bursts of edits write once.
    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self._save_task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            _ = this.update(cx, |this, cx| this.save(cx));
        });
    }

    fn save(&self, cx: &App) {
        let file = LayoutFile {
            dock: Some(self.area.read(cx).dump(cx)),
            sections: HashMap::new(),
        };
        let result = serde_json::to_string_pretty(&file)
            .map_err(|err| err.to_string())
            .and_then(|json| {
                std::fs::create_dir_all(LAYOUT_DIR)
                    .and_then(|_| std::fs::write(LAYOUT_FILE, json))
                    .map_err(|err| err.to_string())
            });
        if let Err(err) = result {
            eprintln!("failed to save layout to {LAYOUT_FILE}: {err}");
        }
    }

    /// Loads the saved layout; false (fall back to the default) when missing,
    /// invalid or from an older schema.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Ok(text) = std::fs::read_to_string(LAYOUT_FILE) else {
            return false;
        };
        let file: LayoutFile = match serde_json::from_str(&text) {
            Ok(file) => file,
            Err(err) => {
                eprintln!("invalid layout {LAYOUT_FILE}: {err}; using default");
                return false;
            }
        };
        let Some(state) = file
            .into_dock()
            .filter(|state| state.version == Some(LAYOUT_VERSION))
        else {
            return false;
        };
        match self
            .area
            .update(cx, |area, cx| area.load(state, window, cx))
        {
            Ok(()) => true,
            Err(err) => {
                eprintln!("failed to load layout {LAYOUT_FILE}: {err}; using default");
                false
            }
        }
    }
}

/// Where a panel kind goes when none of its kind is open; an extension
/// panel (`extension:<key>`) where its manifest says (never the Explorer
/// section unless it is one of its tabs).
fn default_placement(name: &str, cx: &App) -> DockPlacement {
    if let Some(key) = name.strip_prefix(ADD_PREFIX) {
        let registry = crate::extensions::registry(cx);
        return match registry
            .panel(key)
            .map(|(_, panel)| panel.placement().to_string())
        {
            Some(place) if place == "right" => DockPlacement::Right,
            Some(place) if place == "bottom" => DockPlacement::Bottom,
            Some(place) if place == "left" => DockPlacement::Right,
            _ => DockPlacement::Center,
        };
    }
    match name {
        "Explorer" => DockPlacement::Left,
        "Chat" => DockPlacement::Right,
        "Terminal" => DockPlacement::Bottom,
        _ => DockPlacement::Center,
    }
}

/// "Chat", then "Chat 2", "Chat 3", ...: the first name not in use.
fn next_title(name: &str, titles: &[SharedString]) -> String {
    (1..)
        .map(|n| {
            if n == 1 {
                name.to_string()
            } else {
                format!("{name} {n}")
            }
        })
        .find(|title| !titles.iter().any(|used| used.as_ref() == title))
        .unwrap_or_else(|| name.to_string())
}

/// The first tab group of a region, if it has one.
fn first_group(tree: &PaneTree) -> Option<NodeId> {
    let mut tabs: Option<NodeId> = None;
    tree.root().walk(&mut |node| {
        if tabs.is_none() && matches!(node.kind(), PaneRef::Tabs { .. }) {
            tabs = Some(node.id());
        }
    });
    tabs
}

/// Where a moved panel lands: the first tab group of the target, or a new group
/// when the target is empty.
fn insert_target(tree: &PaneTree) -> InsertTarget {
    match first_group(tree) {
        Some(node) => InsertTarget::Tabs {
            node,
            ix: None,
            activate: true,
        },
        None => InsertTarget::Split {
            node: tree.root().id(),
            placement: Placement::Right,
            size: None,
        },
    }
}

/// A new panel for extension panel `key`: a graph panel, a web page (with
/// the `webview` feature) or an extension panel.
fn extension_panel_handle(key: &str, title: String, cx: &mut App) -> Arc<dyn BasePanelView> {
    let graph = crate::extensions::registry(cx)
        .panel(key)
        .is_some_and(|(_, panel)| panel.kind == carbon_core::extension::PanelKind::Graph);
    if graph {
        return panel_handle(crate::graph_panel::GraphPanel::new(
            key.to_string(),
            title,
            None,
            cx,
        ));
    }
    #[cfg(feature = "webview")]
    {
        use carbon_core::extension::{ExtensionId, PanelKind};
        let registry = crate::extensions::registry(cx);
        if let Some((loaded, panel)) = registry.panel(key)
            && panel.kind == PanelKind::Webview
            && let Some(page) = panel.page.clone()
        {
            let source = crate::webview_panel::PageSource {
                extension: ExtensionId::new(loaded.def.id.as_str()),
                page,
            };
            return panel_handle(crate::webview_panel::WebviewPanel::new(
                title,
                Some(source),
                cx,
            ));
        }
    }
    panel_handle(ExtensionPanel::new(key.to_string(), title, cx))
}

#[cfg(feature = "webview")]
impl Workbench {
    /// Shows each webview panel only where nothing GPUI draws must cover it
    /// (a native page sits above GPUI): its dock is open, no other group is
    /// zoomed, and no overlay holds focus. An overlay is a dialog, or focus
    /// outside every panel and not at the window root: the palette, a
    /// title-bar or tab menu. Background tabs hide themselves (`set_active`).
    /// Runs every frame.
    fn sync_webviews(&self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::webview_panel::WebviewPanel;
        let mut panels = Vec::new();
        {
            let area = self.area.read(cx);
            // A zoomed group fills the area: only its own pages stay.
            let zoomed = area.zoomed_group();
            for placement in CENTER_AND_DOCKS {
                let Some(tree) = area.layout(placement) else {
                    continue;
                };
                let open = placement == DockPlacement::Center || area.is_dock_open(placement);
                for id in tree.panels() {
                    if let Some(panel) = area
                        .panel(id)
                        .and_then(|view| view.view().downcast::<WebviewPanel>().ok())
                    {
                        let shown =
                            zoomed.is_none_or(|group| tree.find_panel_node(id) == Some(group));
                        panels.push((panel, open && shown));
                    }
                }
            }
        }
        if panels.is_empty() {
            return;
        }
        let overlay = window.has_active_dialog(cx)
            || window.has_active_sheet(cx)
            || window.focused(cx).is_some_and(|focused| {
                let area = self.area.read(cx).focus_handle(cx);
                !focused.contains(&area, window) && self.focused_panel(window, cx).is_none()
            });
        for (panel, open) in panels {
            panel.update(cx, |panel, cx| panel.sync(open && !overlay, cx));
        }
    }
}

impl Render for Workbench {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(feature = "webview")]
        self.sync_webviews(_window, _cx);
        div().size_full().child(self.area.clone())
    }
}
