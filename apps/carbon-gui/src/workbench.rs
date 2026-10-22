//! Main dock area. Tier T1 (kit `DockArea`: drag, snap, split, resize, close, zoom,
//! persist) plus T2: named layouts, task presets, keyboard move/resize, auto-hide docks.
//! Files live in the build dir for now; they move to the OS user dir later.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
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
use carbon_core::workflow::{
    InstallKind, InstallPlan, InstallRecord, LayoutFile, Origin, Package, SavedWorkflow, StateFile,
    WorkflowId, finish_entering, installed_hash, start_entering, verify_installed,
};
use gpui_kit::component::dock::panel_handle;

use crate::chat_panel::ChatPanel;
use crate::editor_panel::EditorPanel;
use crate::explorer_panel::ExplorerPanel;
use crate::layouts::{LayoutSpec, LayoutStore, MAX_SAVED, PanelKind, Preset, SavedLayout, Side};
use crate::menus::{Direction, MenuState};
use crate::style::fixed;
use crate::terminal_panel::TerminalPanel;
use crate::test_panel::TestPanel;
use crate::workflows;
use carbon_core::terminal::shells;

/// Saved-layout schema version; bump when old layouts can no longer load.
/// 2: Explorer is a real panel (v1 layouts hold a placeholder "Explorer").
const LAYOUT_VERSION: usize = 2;
const LAYOUT_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/carbon-gui/layout.json"
);
const SAVE_DELAY: Duration = Duration::from_millis(500);
/// How long a workflow must stay open before its opening counts as finished
/// (crash-loop guard, `workflow-system.md` s4.8).
const ENTER_CONFIRM_DELAY: Duration = Duration::from_secs(2);
const CENTER_AND_DOCKS: [DockPlacement; 4] = [
    DockPlacement::Center,
    DockPlacement::Left,
    DockPlacement::Right,
    DockPlacement::Bottom,
];

/// The Explorer section: the Explorer and the workflow's own tabs beside it.
/// Fixed (`set_dock_fixed`): they rearrange and split inside it, but none
/// closes or leaves, and no other panel comes in. It still collapses.
const EXPLORER_SECTION: DockPlacement = DockPlacement::Left;

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
        if placement == Some(EXPLORER_SECTION) {
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
    /// What `state.json` holds: every workflow's last layout (the active one's
    /// current layout lives in `area` until saved), disabled and quarantined
    /// workflows, including ids the registry no longer has.
    state: StateFile,
    store: LayoutStore,
    /// Messages from restoring, shown once the window is up.
    notices: Vec<String>,
    _layout_sub: Subscription,
    _save_task: Task<()>,
    _enter_task: Task<()>,
    _verify_task: Task<()>,
}

impl EventEmitter<MenusChanged> for Workbench {}

impl Workbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (area, skin) = DockSkin::dock_area("carbon-main", Some(LAYOUT_VERSION), window, cx);
        skin.set_close_button_visible(true, cx);
        // No dock collapse buttons in tab bars (user): docks open and close by
        // dragging their edge, the View menu and Ctrl+B / Ctrl+Alt+B / Ctrl+J.
        skin.set_toggle_button_visible(false, cx);
        area.update(cx, |area, cx| {
            area.set_dock_fixed(EXPLORER_SECTION, true, window, cx)
        });

        let _layout_sub = cx.subscribe_in(&area, window, |this, _, event, window, cx| {
            if let DockEvent::LayoutChanged = event {
                this.repair_layout(window, cx);
                this.schedule_save(cx);
            }
        });

        let mut this = Self {
            area,
            section: default_workflow(),
            state: StateFile::default(),
            store: LayoutStore::default(),
            notices: Vec::new(),
            _layout_sub,
            _save_task: Task::ready(()),
            _enter_task: Task::ready(()),
            _verify_task: Task::ready(()),
        };
        this.restore(window, cx);
        this.close_empty_docks(window, cx);
        this.verify_installed(window, cx);
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
            // Never into the Explorer section, even beside one of its kind.
            let same: Vec<_> = panels
                .iter()
                .filter(|(_, placement, kind, _)| {
                    kind.as_deref() == Some(name) && *placement != EXPLORER_SECTION
                })
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
            workflow_commands: Vec::new(),
        }
    }

    // ---- workflows (Section Bar) -------------------------------------------

    /// Keeps the current workflow's layout, then shows the target's last layout
    /// (or its default the first time).
    pub fn switch_section(&mut self, id: &WorkflowId, window: &mut Window, cx: &mut Context<Self>) {
        if *id == self.section || !workflows::registry().is_available(id) {
            return;
        }
        self.park_current(cx);
        self.section = id.clone();
        self.store = self.store_for(id);
        self.enter(window, cx);
        for notice in std::mem::take(&mut self.notices) {
            window.push_notification(notice, cx);
        }
        self.schedule_save(cx);
        cx.emit(MenusChanged);
    }

    /// Messages from start-up (a reset or unreadable layout) for the window.
    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    // ---- Manage Workflows ----------------------------------------------------

    /// Checks every installed package against the hash recorded at install,
    /// off the UI thread. A changed one is turned off until Reinstall or Keep
    /// Changes (its flag is saved, so it stays off from the next start on);
    /// one that matches again is turned back on.
    fn verify_installed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let registry = workflows::registry();
        let checks: Vec<(WorkflowId, PathBuf, String)> = self
            .state
            .installs
            .iter()
            .filter(|(id, _)| matches!(registry.origin(id), Some(Origin::Installed(_))))
            .filter_map(|(id, record)| {
                let dir = registry.package_dir(id)?.to_path_buf();
                Some((id.clone(), dir, record.hash.clone()))
            })
            .collect();
        if checks.is_empty() {
            return;
        }
        let results = cx.background_executor().spawn(async move {
            checks
                .into_iter()
                .map(|(id, dir, hash)| (id, verify_installed(&dir, &hash).err()))
                .collect::<Vec<_>>()
        });
        self._verify_task = cx.spawn_in(window, async move |this, cx| {
            let results = results.await;
            _ = this.update_in(cx, |this, window, cx| {
                let registry = workflows::registry();
                let mut names = Vec::new();
                let mut any = false;
                for (id, problem) in results {
                    match problem {
                        Some(problem) => {
                            if this.state.changed.insert(id.clone(), problem).is_none() {
                                any = true;
                                names.push(
                                    registry
                                        .get(&id)
                                        .map_or(id.to_string(), |def| format!("\"{}\"", def.name)),
                                );
                            }
                        }
                        None => any |= this.state.changed.remove(&id).is_some(),
                    }
                }
                if any {
                    this.workflows_changed(window, cx);
                }
                if !names.is_empty() {
                    window.push_notification(
                        format!(
                            "Turned off {}: its files changed since it was installed. \
                             Manage Workflows can reinstall it or keep the changes.",
                            names.join(", ")
                        ),
                        cx,
                    );
                }
            });
        });
    }

    /// Turned off because its files changed since install, and why.
    pub fn changed_reason(&self, id: &WorkflowId) -> Option<String> {
        self.state.changed.get(id).cloned()
    }

    /// Trusts a changed package's files as they are now: records their hash
    /// (no longer pinned or signed) and turns it back on.
    pub fn keep_changes(&mut self, id: &WorkflowId, window: &mut Window, cx: &mut Context<Self>) {
        let registry = workflows::registry();
        let Some(dir) = registry.package_dir(id) else {
            return;
        };
        let hash = match installed_hash(dir) {
            Ok(hash) => hash,
            Err(err) => {
                window.push_notification(format!("Could not read its files: {err}"), cx);
                return;
            }
        };
        if let Some(record) = self.state.installs.get_mut(id) {
            record.hash = hash;
            record.pinned = false;
            record.signed = false;
        }
        self.state.changed.remove(id);
        self.workflows_changed(window, cx);
    }

    /// Turned off by the user, and the quarantine reason if any.
    pub fn workflow_flags(&self, id: &WorkflowId) -> (bool, Option<String>) {
        (
            self.state.disabled.contains(id),
            self.state.quarantined.get(id).cloned(),
        )
    }

    /// Has a saved layout (Reset would change something besides an overlay).
    pub fn has_saved_layout(&self, id: &WorkflowId) -> bool {
        *id == self.section || self.state.workflows.contains_key(id)
    }

    /// Turns a workflow on or off. Turning on also lifts a quarantine.
    pub fn set_workflow_on(
        &mut self,
        id: &WorkflowId,
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workflows::registry().can_turn_off(id) {
            return;
        }
        if on {
            self.state.disabled.remove(id);
            self.state.quarantined.remove(id);
        } else {
            self.state.disabled.insert(id.clone());
        }
        self.workflows_changed(window, cx);
    }

    /// Deletes the workflow's overlay and saved layout; its named layouts stay.
    pub fn reset_workflow(&mut self, id: &WorkflowId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dirs) = workflows::dirs()
            && let Err(err) = dirs.remove_overlay(id.as_str())
        {
            window.push_notification(format!("Could not reset: {err}"), cx);
            return;
        }
        self.state.workflows.remove(id);
        if *id == self.section {
            self.reload_registry();
            self.apply_workflow_default(window, cx);
        }
        self.workflows_changed(window, cx);
    }

    /// Copies the workflow (as it is now, overlay included) to a new user
    /// workflow and returns its id.
    pub fn duplicate_workflow(
        &mut self,
        id: &WorkflowId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<WorkflowId> {
        let dirs = workflows::dirs()?;
        let registry = workflows::registry();
        let def = registry.get(id)?;
        let name = format!("{} (copy)", def.name);
        let copy = def.duplicate(registry.free_user_id(&name), name);
        match dirs.create_user_workflow(&copy, registry.package_dir(id)) {
            Ok(_) => {
                self.workflows_changed(window, cx);
                window.push_notification(format!("Created \"{}\".", copy.name), cx);
                Some(copy.id)
            }
            Err(err) => {
                window.push_notification(format!("Could not duplicate: {err}"), cx);
                None
            }
        }
    }

    /// Deletes a user workflow (and its saved state) or an installed package
    /// (its saved state stays for a reinstall).
    pub fn remove_workflow(
        &mut self,
        id: &WorkflowId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let registry = workflows::registry();
        let (Some(dirs), Some(origin)) = (workflows::dirs(), registry.origin(id)) else {
            return;
        };
        if let Err(err) = dirs.remove_workflow(origin) {
            window.push_notification(format!("Could not remove: {err}"), cx);
            return;
        }
        if matches!(origin, Origin::User(_)) {
            self.state.workflows.remove(id);
            self.state.layouts.remove(id);
            self.state.disabled.remove(id);
            self.state.quarantined.remove(id);
            if let Err(err) = dirs.remove_overlay(id.as_str()) {
                eprintln!("could not remove the overlay of {id}: {err}");
            }
        }
        // A reinstall asks again: its files may differ.
        self.state.grants.remove(id);
        self.state.installs.remove(id);
        self.state.changed.remove(id);
        self.workflows_changed(window, cx);
    }

    /// Installs a checked package whose plan the user accepted; its
    /// capabilities are granted. An install lifts a quarantine; a changed
    /// default layout replaces the saved one.
    pub fn install_package(
        &mut self,
        package: &Package,
        plan: &InstallPlan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dirs) = workflows::dirs() else {
            return;
        };
        if !plan.problems.is_empty() {
            return;
        }
        if let Err(err) = dirs.install(package) {
            window.push_notification(format!("Could not install: {err}"), cx);
            return;
        }
        let def = &package.def;
        let id = def.id.clone();
        self.state
            .grants
            .insert(id.clone(), def.capabilities.iter().cloned().collect());
        self.state.installs.insert(id.clone(), package.record());
        self.state.changed.remove(&id);
        self.state.quarantined.remove(&id);
        // A layout kept from another version (also after an uninstall) goes
        // quietly: the user just chose this version.
        let stale = self
            .state
            .workflows
            .get(&id)
            .is_some_and(|saved| !saved.matches(def));
        if plan.layout_changed || stale {
            self.state.workflows.remove(&id);
            if id == self.section {
                self.reload_registry();
                self.apply_workflow_default(window, cx);
            }
        }
        self.workflows_changed(window, cx);
        let done = match &plan.kind {
            InstallKind::Update { from } => {
                format!("Updated \"{}\" from {from} to {}.", def.name, def.version)
            }
            InstallKind::Reinstall => format!("Reinstalled \"{}\" {}.", def.name, def.version),
            _ => format!("Installed \"{}\" {}.", def.name, def.version),
        };
        window.push_notification(done, cx);
    }

    /// Where installed package `id` came from, if it was installed through Carbon.
    pub fn install_record(&self, id: &WorkflowId) -> Option<InstallRecord> {
        self.state.installs.get(id).cloned()
    }

    /// Capabilities granted to `id`.
    pub fn grants(&self, id: &WorkflowId) -> BTreeSet<String> {
        self.state.grants.get(id).cloned().unwrap_or_default()
    }

    /// Grants all capabilities `id` asks for, or revokes them all.
    pub fn set_grants(&mut self, id: &WorkflowId, allow: bool, cx: &mut Context<Self>) {
        let asked = workflows::registry()
            .get(id)
            .map(|def| def.capabilities.iter().cloned().collect())
            .unwrap_or_default();
        if allow {
            self.state.grants.insert(id.clone(), asked);
        } else {
            self.state.grants.remove(id);
        }
        self.schedule_save(cx);
        cx.notify();
    }

    /// Reads the workflow files again (after editing them by hand).
    pub fn reload_workflows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.workflows_changed(window, cx);
    }

    fn reload_registry(&self) {
        workflows::reload(&self.state);
    }

    /// After a Manage Workflows change: rebuild the registry; leave the
    /// active workflow if it is gone or off; save; refresh bar and menus.
    fn workflows_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reload_registry();
        if !workflows::registry().is_available(&self.section) {
            // Its layout is kept in `state` unless it was removed.
            if workflows::registry().get(&self.section).is_some() {
                self.park_current(cx);
            }
            self.section = default_workflow();
            self.store = self.store_for(&self.section);
            self.enter(window, cx);
        }
        self.schedule_save(cx);
        cx.emit(MenusChanged);
        cx.notify();
    }

    /// A workflow's named layouts and auto-hide flags. The Editor takes the
    /// old global `layouts.json` the first time.
    fn store_for(&self, id: &WorkflowId) -> LayoutStore {
        match self.state.layouts.get(id) {
            Some(value) => LayoutStore::from_value(value),
            None if *id == WorkflowId::editor() => LayoutStore::load_legacy(),
            None => LayoutStore::default(),
        }
    }

    /// Records the active workflow's current layout and named layouts in `state`.
    fn park_current(&mut self, cx: &App) {
        self.state
            .layouts
            .insert(self.section.clone(), self.store.to_value());
        let registry = workflows::registry();
        let Some(def) = registry.get(&self.section) else {
            return;
        };
        match serde_json::to_value(self.area.read(cx).dump(cx)) {
            Ok(dock) => {
                self.state
                    .workflows
                    .insert(self.section.clone(), SavedWorkflow::new(def, dock));
            }
            Err(err) => eprintln!("cannot keep the layout of {}: {err}", self.section),
        }
    }

    /// Shows the active workflow: its saved layout, else its default. Marks
    /// the opening for the crash-loop guard until it has lasted a moment.
    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dirs = workflows::dirs();
        if let Some(dirs) = dirs
            && let Err(err) = start_entering(dirs, &self.section)
        {
            eprintln!("crash guard: {err}");
        }
        if !self.load_saved(window, cx) {
            self.apply_workflow_default(window, cx);
        }
        self.keep_explorer_section(window, cx);
        // Auto-hide docks start hidden, like JetBrains unpinned tool windows.
        for side in self.store.auto_hide.clone() {
            self.set_dock_open(side.placement(), false, window, cx);
        }
        if let Some(dirs) = dirs.cloned() {
            self._enter_task = cx.spawn(async move |_, cx| {
                cx.background_executor().timer(ENTER_CONFIRM_DELAY).await;
                finish_entering(&dirs);
            });
        }
    }

    /// Loads the active workflow's saved layout; false when there is none or
    /// it cannot be used (then the default applies). A layout saved from an
    /// older default is dropped with a notice (no merge yet).
    fn load_saved(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let registry = workflows::registry();
        let (Some(def), Some(saved)) = (
            registry.get(&self.section),
            self.state.workflows.get(&self.section),
        ) else {
            return false;
        };
        if !saved.matches(def) {
            self.notices.push(format!(
                "The default layout of \"{}\" changed, so its layout was reset.",
                def.name
            ));
            self.state.workflows.remove(&self.section);
            return false;
        }
        let dock = match serde_json::from_value::<DockAreaState>(saved.dock.clone()) {
            Ok(dock) if dock.version == Some(LAYOUT_VERSION) => dock,
            // An older dock format: silently the default, as before workflows.
            Ok(_) => return false,
            Err(err) => {
                eprintln!("saved layout of {} is broken: {err}", self.section);
                self.notices.push(format!(
                    "The saved layout of \"{}\" was broken, so it was reset.",
                    def.name
                ));
                return false;
            }
        };
        match self.area.update(cx, |area, cx| area.load(dock, window, cx)) {
            Ok(()) => true,
            Err(err) => {
                eprintln!("failed to load the layout of {}: {err}", self.section);
                false
            }
        }
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
    /// a dock whose last panel left closes; the Explorer section holds the
    /// Explorer and its own tabs.
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

    /// The Explorer section holds the Explorer, first, and the active
    /// workflow's own section tabs, nothing else. A layout saved before the
    /// section was fixed is put right: a missing Explorer or section tab
    /// comes back, an Explorer elsewhere moves in, and other panels move to
    /// where their kind goes.
    fn keep_explorer_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The section's own tabs besides the Explorer, by kind.
        let mut missing: Vec<PanelKind> = workflow_default(&self.section)
            .and_then(|spec| spec.docks.into_iter().find(|dock| dock.side == Side::Left))
            .map(|dock| dock.panels)
            .unwrap_or_default();
        missing.retain(|kind| *kind != PanelKind::Explorer);
        let own: Vec<&'static str> = missing.iter().map(PanelKind::panel_name).collect();
        let mut explorers = Vec::new();
        let mut strays = Vec::new();
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
                    if name == crate::explorer_panel::PANEL_NAME {
                        explorers.push((id, placement, view.view()));
                    } else if placement == EXPLORER_SECTION {
                        match missing.iter().position(|kind| kind.panel_name() == name) {
                            Some(ix) => _ = missing.remove(ix),
                            None if !own.contains(&name) => {
                                strays.push((id, default_placement(name)))
                            }
                            None => {}
                        }
                    }
                }
            }
        }
        let explorer_in_place = explorers
            .first()
            .is_some_and(|(_, placement, _)| *placement == EXPLORER_SECTION);
        if explorers.len() == 1 && explorer_in_place && strays.is_empty() && missing.is_empty() {
            return;
        }
        let missing: Vec<_> = missing.iter().map(|kind| kind.build(cx)).collect();

        self.area.update(cx, |area, cx| {
            // One Explorer: extra ones (from an older layout) go.
            for (_, _, view) in explorers.iter().skip(1) {
                if let Ok(extra) = view.clone().downcast::<ExplorerPanel>() {
                    area.remove_panel(extra, window, cx);
                }
            }
            if area.layout(EXPLORER_SECTION).is_none() {
                area.set_dock(EXPLORER_SECTION, DockLayout::tabs(), window, cx);
                area.set_dock_size(EXPLORER_SECTION, Side::Left.default_size(), window, cx);
            }
            let explorer = match explorers.first() {
                Some((id, ..)) => *id,
                None => {
                    let handle = panel_handle(ExplorerPanel::new(cx));
                    let id = handle.panel_id(cx);
                    area.add_panel_view(handle, EXPLORER_SECTION, None, window, cx);
                    id
                }
            };
            if !explorer_in_place
                && let Some(node) = area.layout(EXPLORER_SECTION).and_then(first_group)
            {
                let target = InsertTarget::Tabs {
                    node,
                    ix: Some(0),
                    activate: true,
                };
                area.move_panel(explorer, target, window, cx);
            }
            for panel in missing {
                area.add_panel_view(panel, EXPLORER_SECTION, None, window, cx);
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
        self.schedule_save(cx);
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
        // The Explorer section keeps its panels and takes no others.
        if from == to || from == EXPLORER_SECTION || to == EXPLORER_SECTION {
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
        // Alone in the editor area there is nothing to sit beside; the
        // Explorer section keeps its panels.
        if from == EXPLORER_SECTION
            || (from == DockPlacement::Center && center.panels().eq([panel]))
        {
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

    /// A preset arranges the center and the right and bottom docks; the
    /// Explorer section stays the workflow's.
    pub fn apply_preset(&mut self, preset: Preset, window: &mut Window, cx: &mut Context<Self>) {
        let (center, mut docks) = preset.build(cx);
        docks.retain(|(side, ..)| side.placement() != EXPLORER_SECTION);
        self.apply_built((center, docks), window, cx);
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
        self.schedule_save(cx);
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
        self.schedule_save(cx);
        self.keep_explorer_section(window, cx);
        self.close_empty_docks(window, cx);
        cx.emit(MenusChanged);
    }

    pub fn delete_layout(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.store.saved.len() {
            self.store.saved.remove(index);
            self.schedule_save(cx);
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

    /// Writes `state.json`. Safe mode (no workflow folder) writes nothing.
    fn save(&mut self, cx: &App) {
        let Some(dirs) = workflows::dirs() else {
            return;
        };
        self.park_current(cx);
        self.state.active = Some(self.section.clone());
        if let Err(err) = self.state.save(&dirs.state_file()) {
            eprintln!("failed to save workflow layouts: {err}");
        }
    }

    /// Takes the state read at start (or, the first time, migrates the Phase 1
    /// `layout.json`, which stays untouched for older builds), then shows the
    /// active workflow.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if workflows::dirs().is_some() {
            self.state = workflows::take_state().unwrap_or_else(migrate_layout_file);
        }
        self.section = self
            .state
            .active
            .clone()
            .filter(|id| workflows::registry().is_available(id))
            .unwrap_or_else(default_workflow);
        self.store = self.store_for(&self.section);
        self.enter(window, cx);
    }
}

/// `state.json` from the Phase 1 `layout.json`, if there is one. Layouts are
/// taken as made from today's defaults.
fn migrate_layout_file() -> StateFile {
    let mut state = StateFile::default();
    let Ok(text) = std::fs::read_to_string(LAYOUT_FILE) else {
        return state;
    };
    let file: LayoutFile<serde_json::Value> = match serde_json::from_str(&text) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("invalid layout {LAYOUT_FILE}: {err}; not migrated");
            return state;
        }
    };
    let registry = workflows::registry();
    let (active, docks) = file.into_parts(&registry);
    state.active = active;
    for (id, dock) in docks {
        let saved = match registry.get(&id) {
            Some(def) => SavedWorkflow::new(def, dock),
            // Unknown workflow: kept; its fingerprint never matches, so it
            // starts from its default if it ever comes back.
            None => SavedWorkflow {
                dock,
                base_fingerprint: String::new(),
                base_panels: Vec::new(),
            },
        };
        state.workflows.insert(id, saved);
    }
    state
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

#[cfg(feature = "webview")]
impl Workbench {
    /// Shows each webview panel only where nothing GPUI draws must cover it
    /// (a native page sits above GPUI): its dock is open, no other group is
    /// zoomed, and no overlay holds focus. An overlay is a dialog, or focus outside every panel and
    /// not at the window root: the palette, a title-bar or tab menu.
    /// Background tabs hide themselves (`set_active`). Runs every frame.
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
