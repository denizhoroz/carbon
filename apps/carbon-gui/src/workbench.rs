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

use carbon_core::BufferId;
use carbon_core::workflow::{LayoutFile, WorkflowId};
use gpui_kit::component::dock::panel_handle;

use crate::chat_panel::ChatPanel;
use crate::editor_panel::EditorPanel;
use crate::explorer_panel::ExplorerPanel;
use crate::layouts::{LayoutSpec, LayoutStore, MAX_SAVED, Preset, SavedLayout, Side};
use crate::menus::{Direction, MenuState};
use crate::style::fixed;
use crate::terminal_panel::TerminalPanel;
use crate::test_panel::TestPanel;
use crate::workflows::{self, SafeMode};
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
/// Reports a failure as a notification.
pub fn open_file_at(path: &Path, window: &mut Window, cx: &mut App) {
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

/// Closes a panel's tab (Explorer, Chat, Terminal, ...; editors use
/// `close_editor`). The center never goes blank: closing its last panel
/// leaves an empty editor. Deferred: the caller is inside the panel's update.
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

/// Section, saved layouts or auto-hide flags changed; the owner rebuilds the
/// menus and updates the Section Bar.
pub struct MenusChanged;

/// The layout a workflow starts with (and resets to); `None` for an id the
/// registry does not know.
fn workflow_default(id: &WorkflowId) -> Option<LayoutSpec> {
    workflows::registry().get(id).map(LayoutSpec::of_workflow)
}

/// The workflow to open when none is saved (or the saved one is gone).
fn default_workflow() -> WorkflowId {
    workflows::registry()
        .default_workflow()
        .map(|def| def.id.clone())
        .unwrap_or_else(WorkflowId::editor)
}

pub struct Workbench {
    area: Entity<DockArea>,
    /// The active workflow.
    section: WorkflowId,
    /// Layouts of workflows other than the active one (the active one lives in
    /// `area`), including ones the registry no longer has.
    section_layouts: HashMap<WorkflowId, DockAreaState>,
    store: LayoutStore,
    _layout_sub: Subscription,
    _save_task: Task<()>,
}

impl EventEmitter<MenusChanged> for Workbench {}

impl Workbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (area, skin) = DockSkin::dock_area("carbon-main", Some(LAYOUT_VERSION), window, cx);
        skin.set_close_button_visible(true, cx);
        // No dock collapse buttons in tab bars (user): docks open and close by
        // dragging their edge, the View menu and Ctrl+B / Ctrl+Alt+B / Ctrl+J.
        skin.set_toggle_button_visible(false, cx);

        let _layout_sub = cx.subscribe_in(&area, window, |this, _, event, window, cx| {
            if let DockEvent::LayoutChanged = event {
                this.repair_layout(window, cx);
                this.schedule_save(cx);
            }
        });

        let mut this = Self {
            area,
            section: default_workflow(),
            section_layouts: HashMap::new(),
            store: LayoutStore::load(),
            _layout_sub,
            _save_task: Task::ready(()),
        };
        if !this.restore(window, cx) {
            this.apply_workflow_default(window, cx);
        }
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
                        });
                    let explorer = view.downcast::<ExplorerPanel>().is_ok();
                    panels.push((id, placement, kind, explorer));
                }
            }
        }
        let (id, placement, handle) = if name == "Explorer" {
            match panels.iter().find(|(.., explorer)| *explorer) {
                // There is one Explorer: show it.
                Some(&(id, placement, ..)) => (id, placement, None),
                None => {
                    let handle = panel_handle(ExplorerPanel::new(cx));
                    (handle.panel_id(cx), DockPlacement::Left, Some(handle))
                }
            }
        } else {
            let same: Vec<_> = panels
                .iter()
                .filter(|(_, _, kind, _)| kind.as_deref() == Some(name))
                .collect();
            let placement = same
                .first()
                .map(|(_, placement, ..)| *placement)
                .unwrap_or_else(|| default_placement(name));
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
            section: self.section.clone(),
            has_workspace: false,
            saved: self.store.names(),
            auto_hide: self.store.auto_hide.clone(),
        }
    }

    // ---- workflows (Section Bar) -------------------------------------------

    /// Keeps the current workflow's layout, then shows the target's last layout
    /// (or its default the first time).
    pub fn switch_section(&mut self, id: &WorkflowId, window: &mut Window, cx: &mut Context<Self>) {
        if *id == self.section || workflows::registry().get(id).is_none() {
            return;
        }
        let current = self.area.read(cx).dump(cx);
        let previous = std::mem::replace(&mut self.section, id.clone());
        self.section_layouts.insert(previous, current);
        let loaded = self.section_layouts.remove(id).is_some_and(|state| {
            self.area
                .update(cx, |area, cx| area.load(state, window, cx))
                .is_ok()
        });
        if !loaded {
            self.apply_workflow_default(window, cx);
        }
        self.schedule_save(cx);
        cx.emit(MenusChanged);
    }

    pub fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_workflow_default(window, cx);
    }

    /// Lays out the active workflow as its manifest describes.
    fn apply_workflow_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let spec = workflow_default(&self.section).unwrap_or_else(LayoutSpec::code);
        let built = spec.build(cx);
        self.apply_built(built, window, cx);
        self.close_empty_docks(window, cx);
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
    /// a dock whose last panel left closes.
    fn repair_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let to = side.map_or(DockPlacement::Center, Side::placement);
        if from == to {
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
        let Some(center) = self.area.read(cx).layout(DockPlacement::Center) else {
            return;
        };
        // Alone in the editor area there is nothing to sit beside.
        if from == DockPlacement::Center && center.panels().eq([panel]) {
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
        let built = preset.build(cx);
        self.apply_built(built, window, cx);
    }

    fn apply_built(
        &mut self,
        (center, docks): (DockLayout, Vec<(Side, DockLayout, Pixels, bool)>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        // Safe mode leaves the saved layouts as they were.
        if cx.try_global::<SafeMode>().is_some_and(|safe| safe.0) {
            return;
        }
        let mut states = self.section_layouts.clone();
        states.insert(self.section.clone(), self.area.read(cx).dump(cx));
        let file = LayoutFile::new(workflows::registry(), &self.section, states);
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

    /// Loads the saved workflow layouts and shows the active one; false (fall
    /// back to the workflow default) when missing, invalid or in safe mode.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if cx.try_global::<SafeMode>().is_some_and(|safe| safe.0) {
            return false;
        }
        let Ok(text) = std::fs::read_to_string(LAYOUT_FILE) else {
            return false;
        };
        let file: LayoutFile<DockAreaState> = match serde_json::from_str(&text) {
            Ok(file) => file,
            Err(err) => {
                eprintln!("invalid layout {LAYOUT_FILE}: {err}; using default");
                return false;
            }
        };
        let (active, mut states) = file.into_parts(workflows::registry());
        self.section = active.unwrap_or_else(default_workflow);
        // Layouts from an older schema are dropped; workflows fall back to defaults.
        states.retain(|_, state| state.version == Some(LAYOUT_VERSION));
        let state = states.remove(&self.section);
        self.section_layouts = states;
        let Some(state) = state else {
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

/// Where a panel kind goes when none of its kind is open.
fn default_placement(name: &str) -> DockPlacement {
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

/// Where a moved panel lands: the first tab group of the target, or a new group
/// when the target is empty.
fn insert_target(tree: &PaneTree) -> InsertTarget {
    let mut tabs: Option<NodeId> = None;
    tree.root().walk(&mut |node| {
        if tabs.is_none() && matches!(node.kind(), PaneRef::Tabs { .. }) {
            tabs = Some(node.id());
        }
    });
    match tabs {
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

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.area.clone())
    }
}
