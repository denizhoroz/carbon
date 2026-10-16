//! Main dock area. Tier T1 (kit `DockArea`: drag, snap, split, resize, close, zoom,
//! persist) plus T2: named layouts, task presets, keyboard move/resize, auto-hide docks.
//! Files live in the build dir for now; they move to the OS user dir later.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::Placement;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{
    BasePanelView, DockArea, DockAreaState, DockEvent, DockPlacement, DockSkin, InsertTarget,
    NodeId, PaneRef, PaneTree, PanelId,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt, h_flex};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

use crate::layouts::{LayoutStore, MAX_SAVED, Preset, SavedLayout, Side};
use crate::menus::{Direction, MenuState};
use crate::section_bar::Section;
use crate::style::fixed;

/// Saved-layout schema version; bump when old layouts can no longer load.
const LAYOUT_VERSION: usize = 1;
const LAYOUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/carbon-gui");
const LAYOUT_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/carbon-gui/layout.json");
const SAVE_DELAY: Duration = Duration::from_millis(500);
const CENTER_AND_DOCKS: [DockPlacement; 4] = [
    DockPlacement::Center,
    DockPlacement::Left,
    DockPlacement::Right,
    DockPlacement::Bottom,
];

/// Section, saved layouts or auto-hide flags changed; the owner rebuilds the
/// menus and updates the Section Bar.
pub struct MenusChanged;

/// `layout.json`: the active section and each visited section's last layout.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct LayoutFile {
    active: Option<Section>,
    sections: HashMap<Section, DockAreaState>,
}

/// The layout a section starts with (and resets to).
fn section_preset(section: Section) -> Preset {
    match section {
        Section::Code => Preset::Code,
    }
}

pub struct Workbench {
    area: Entity<DockArea>,
    section: Section,
    /// Layouts of sections other than the active one (the active one lives in `area`).
    section_layouts: HashMap<Section, DockAreaState>,
    store: LayoutStore,
    _layout_sub: Subscription,
    _save_task: Task<()>,
}

impl EventEmitter<MenusChanged> for Workbench {}

impl Workbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (area, skin) = DockSkin::dock_area("carbon-main", Some(LAYOUT_VERSION), window, cx);
        skin.set_close_button_visible(true, cx);

        let _layout_sub = cx.subscribe(&area, |this, _, event: &DockEvent, cx| {
            if let DockEvent::LayoutChanged = event {
                this.schedule_save(cx);
            }
        });

        let mut this = Self {
            area,
            section: Section::Code,
            section_layouts: HashMap::new(),
            store: LayoutStore::load(),
            _layout_sub,
            _save_task: Task::ready(()),
        };
        if !this.restore(window, cx) {
            this.apply_preset(section_preset(this.section), window, cx);
        }
        // Auto-hide docks start hidden, like JetBrains unpinned tool windows.
        for side in this.store.auto_hide.clone() {
            this.set_dock_open(side.placement(), false, window, cx);
        }
        this
    }

    pub fn menu_state(&self) -> MenuState {
        MenuState {
            section: self.section,
            saved: self.store.names(),
            auto_hide: self.store.auto_hide.clone(),
        }
    }

    // ---- sections --------------------------------------------------------

    /// Keeps the current section's layout, then shows the target's last layout
    /// (or its default the first time).
    pub fn switch_section(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if section == self.section {
            return;
        }
        let current = self.area.read(cx).dump(cx);
        self.section_layouts.insert(self.section, current);
        self.section = section;
        let loaded = self.section_layouts.remove(&section).is_some_and(|state| {
            self.area
                .update(cx, |area, cx| area.load(state, window, cx))
                .is_ok()
        });
        if !loaded {
            self.apply_preset(section_preset(section), window, cx);
        }
        self.schedule_save(cx);
        cx.emit(MenusChanged);
    }

    pub fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_preset(section_preset(self.section), window, cx);
    }

    // ---- docks -----------------------------------------------------------

    pub fn toggle_dock(&mut self, side: Side, window: &mut Window, cx: &mut Context<Self>) {
        let placement = side.placement();
        self.area
            .update(cx, |area, cx| area.toggle_dock(placement, window, cx));
        // An auto-hide dock takes focus when shown, so keyboard users land in it.
        if self.store.auto_hide.contains(&side) && self.area.read(cx).is_dock_open(placement) {
            if let Some(panel) = self.first_panel(placement, cx) {
                panel.focus_handle(cx).focus(window, cx);
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

    pub fn resize_dock(&mut self, direction: Direction, window: &mut Window, cx: &mut Context<Self>) {
        let Some(side) = self.focused_panel(window, cx).and_then(|(_, p)| Side::of(p)) else {
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
        let result = self
            .area
            .update(cx, |area, cx| area.load(layout.dock, window, cx));
        if let Err(err) = result {
            window.push_notification(format!("Could not apply \"{}\": {err}", layout.name), cx);
            return;
        }
        self.store.auto_hide = layout.auto_hide;
        self.store.save();
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

        let submit: Rc<dyn Fn(&mut Window, &mut App)> = {
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
        cx.subscribe_in(&input, window, move |_, _, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                on_enter(window, cx);
            }
        })
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
        let mut sections = self.section_layouts.clone();
        sections.insert(self.section, self.area.read(cx).dump(cx));
        let file = LayoutFile {
            active: Some(self.section),
            sections,
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

    /// Loads the saved sections and shows the active one; false (fall back to the
    /// section default) when missing or invalid.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Ok(text) = std::fs::read_to_string(LAYOUT_FILE) else {
            return false;
        };
        let mut file: LayoutFile = match serde_json::from_str(&text) {
            Ok(file) => file,
            Err(err) => {
                eprintln!("invalid layout {LAYOUT_FILE}: {err}; using default");
                return false;
            }
        };
        self.section = file.active.unwrap_or(Section::Code);
        let state = file.sections.remove(&self.section);
        self.section_layouts = file.sections;
        let Some(state) = state else {
            return false;
        };
        match self.area.update(cx, |area, cx| area.load(state, window, cx)) {
            Ok(()) => true,
            Err(err) => {
                eprintln!("failed to load layout {LAYOUT_FILE}: {err}; using default");
                false
            }
        }
    }
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
