//! Graph panels: a node-and-arrow editor for a graph kind an extension
//! declares (`[[graphs]]`, `[[node_types]]`, a `graph` panel), such as the
//! Agents extension's flows (`.agents/documents/agents.md`). One panel shows
//! one graph file in the project; without one it lists the kind's files and
//! makes new ones. Carbon draws and edits the graph (blocks, arrows between
//! ports, pan and zoom, undo, autosave); what the blocks do is up to the
//! extension.
//!
//! Canvas: a GPUI `canvas` paints the grid and arrows; blocks are positioned
//! divs on top (`canvas.rs`). The inspector edits the selected block's fields
//! from its node type (`inspector.rs`). Geometry and hit tests: `geometry.rs`.

mod canvas;
mod files;
mod geometry;
mod inspector;
mod run_view;

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use carbon_core::extension::GraphKind;
use carbon_core::graph::{
    Diagnostic, GraphDoc, GraphEdit, History, NodeTypeSpec, PortRef, Pos, Severity, read_file,
    validate, write_file,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::extensions::host::{AgentsHost, RunEntry};
use crate::style::fixed;
use geometry::View;
use inspector::InspectorInput;

/// Persistence key in saved layouts. Never change it.
pub const PANEL_NAME: &str = "Graph";
/// Key context of the canvas (not the inspector: its boxes take typing).
const KEY_CONTEXT: &str = "GraphCanvas";
/// Saved this long after the last change (DESIGN.md canvas rules).
const AUTOSAVE_DELAY: Duration = Duration::from_millis(1500);
/// How often the file is checked for changes made elsewhere.
const DISK_POLL: Duration = Duration::from_secs(2);

actions!(
    graph,
    [
        Undo,
        Redo,
        DeleteSelection,
        Duplicate,
        SelectAll,
        ZoomIn,
        ZoomOut,
        ZoomFit,
        ZoomReset,
        SaveGraph,
        Cancel
    ]
);

/// Binds the canvas keys and lets saved layouts recreate graph panels.
pub fn register(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-z", Undo, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-shift-z", Redo, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-y", Redo, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", DeleteSelection, Some(KEY_CONTEXT)),
        KeyBinding::new("backspace", DeleteSelection, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-d", Duplicate, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-a", SelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("=", ZoomIn, Some(KEY_CONTEXT)),
        KeyBinding::new("+", ZoomIn, Some(KEY_CONTEXT)),
        KeyBinding::new("-", ZoomOut, Some(KEY_CONTEXT)),
        KeyBinding::new("0", ZoomFit, Some(KEY_CONTEXT)),
        KeyBinding::new("1", ZoomReset, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-s", SaveGraph, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(KEY_CONTEXT)),
    ]);
    register_panel(cx, PANEL_NAME, |build, _, cx| {
        let text = |key: &str| match build.info() {
            PanelInfo::Panel(value) => value[key].as_str().map(str::to_string),
            _ => None,
        };
        let key = text("key").unwrap_or_default();
        let title = text("title").unwrap_or_else(|| "Graph".into());
        let path = text("path").map(PathBuf::from);
        panel_handle(GraphPanel::new(key, title, path, cx))
    });
}

/// Opens `path` in a graph panel when an active extension's graph kind
/// claims its file name (`*.flow.yaml`); false when none does.
pub fn open_graph_file(path: &Path, window: &mut Window, cx: &mut App) -> bool {
    let registry = crate::extensions::registry(cx);
    let Some(kind) = registry.graph_for_path(path) else {
        return false;
    };
    let graph_id = kind.spec.id.clone();
    let Some(panel) = kind
        .loaded
        .def
        .panels
        .iter()
        .find(|p| p.graph.as_deref() == Some(graph_id.as_str()))
    else {
        return false;
    };
    let key = format!("{}/{}", kind.loaded.def.id, panel.id);
    crate::workbench::open_graph(path, key, panel.title.clone(), window, cx);
    true
}

/// What the inspector shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InspectorTab {
    /// The selection (or the graph).
    Edit,
    /// The newest run.
    Run,
}

/// Where the open graph stands on disk.
#[derive(Clone, Debug, PartialEq)]
enum SaveState {
    Saved,
    Unsaved,
    Failed(String),
}

/// What a press on the canvas is doing.
enum Drag {
    /// Moving the selected blocks; `origins` are where they started.
    Move {
        start: Point<Pixels>,
        origins: Vec<(String, Pos)>,
        moved: bool,
    },
    Pan {
        start: Point<Pixels>,
        origin: View,
    },
    /// Drawing an arrow from an output; `to` is the pointer (local px).
    Connect {
        from: PortRef,
        to: Point<Pixels>,
    },
    Marquee {
        start: Point<Pixels>,
        current: Point<Pixels>,
        extend: bool,
    },
}

pub struct GraphPanel {
    /// The panel's key `<extension id>/<panel id>`.
    key: String,
    title: SharedString,
    path: Option<PathBuf>,
    doc: Option<GraphDoc>,
    load_error: Option<String>,
    history: History,
    save_state: SaveState,
    disk_mtime: Option<SystemTime>,
    /// The file changed on disk while there were unsaved edits.
    disk_conflict: bool,
    view: View,
    selected: BTreeSet<String>,
    selected_edge: Option<String>,
    drag: Option<Drag>,
    space_down: bool,
    /// Carbon's checks and the extension's (`check-graph`), together.
    diagnostics: Vec<Diagnostic>,
    /// The extension's own, from its last check.
    ext_diagnostics: Vec<Diagnostic>,
    _check: Task<()>,
    /// Canvas bounds in window coordinates, from the last paint.
    bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Fit the view once the canvas has a size (after opening a file).
    fit_pending: bool,
    inputs: HashMap<String, InspectorInput>,
    input_subs: Vec<Subscription>,
    /// The newest run of this file (from the extension running it).
    run: Option<RunEntry>,
    tab: InspectorTab,
    focus_handle: FocusHandle,
    canvas_focus: FocusHandle,
    _autosave: Task<()>,
    _poll: Task<()>,
    _subs: [Subscription; 2],
}

impl GraphPanel {
    pub fn new(
        key: String,
        title: impl Into<SharedString>,
        path: Option<PathBuf>,
        cx: &mut App,
    ) -> Entity<Self> {
        let title = title.into();
        cx.new(|cx| {
            let mut this = Self {
                key,
                title,
                path: None,
                doc: None,
                load_error: None,
                history: History::new(),
                save_state: SaveState::Saved,
                disk_mtime: None,
                disk_conflict: false,
                view: View {
                    x: 0.,
                    y: 0.,
                    zoom: 1.,
                },
                selected: BTreeSet::new(),
                selected_edge: None,
                drag: None,
                space_down: false,
                diagnostics: Vec::new(),
                ext_diagnostics: Vec::new(),
                _check: Task::ready(()),
                bounds: Rc::new(Cell::new(Bounds::default())),
                fit_pending: false,
                inputs: HashMap::new(),
                input_subs: Vec::new(),
                run: None,
                tab: InspectorTab::Edit,
                focus_handle: cx.focus_handle(),
                canvas_focus: cx.focus_handle(),
                _autosave: Task::ready(()),
                _poll: Task::ready(()),
                _subs: [
                    cx.observe_global::<crate::extensions::Extensions>(|this: &mut Self, cx| {
                        this.revalidate(cx);
                        cx.notify()
                    }),
                    // Runs, questions, transcripts: redraw.
                    cx.observe_global::<AgentsHost>(|_, cx| cx.notify()),
                ],
            };
            if let Some(path) = path {
                this.open(path, cx);
            }
            this.start_poll(cx);
            this
        })
    }

    /// The file this panel shows.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The graph kind and node types from the panel's extension.
    fn kind<'a>(&self, registry: &'a carbon_core::extension::Registry) -> Option<GraphKind<'a>> {
        let (_, panel) = registry.panel(&self.key)?;
        let graph = panel.graph.as_deref()?;
        let extension = self.key.split('/').next()?;
        registry.graph(&format!("{extension}/{graph}"))
    }

    fn types(&self, cx: &App) -> Vec<NodeTypeSpec> {
        let registry = crate::extensions::registry(cx);
        self.kind(&registry)
            .map(|kind| kind.owned_types())
            .unwrap_or_default()
    }

    /// Opens `path`; a file that does not read shows its error.
    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.load_error = None;
        match read_file(&path) {
            Ok(doc) => {
                self.view = View::from(doc.layout.viewport);
                self.doc = Some(doc);
                self.fit_pending = self.view.x == 0. && self.view.y == 0.;
            }
            Err(err) => {
                self.doc = None;
                self.load_error = Some(err);
            }
        }
        self.disk_mtime = mtime(&path);
        self.disk_conflict = false;
        self.path = Some(path);
        self.history = History::new();
        self.save_state = SaveState::Saved;
        self.selected.clear();
        self.selected_edge = None;
        self.clear_inputs();
        self.revalidate(cx);
        cx.notify();
    }

    /// Shows the list of graph files again.
    fn close_file(&mut self, cx: &mut Context<Self>) {
        self.save_now(cx);
        self.path = None;
        self.doc = None;
        self.load_error = None;
        self.clear_inputs();
        cx.notify();
    }

    fn clear_inputs(&mut self) {
        self.inputs.clear();
        self.input_subs.clear();
    }

    fn revalidate(&mut self, cx: &mut Context<Self>) {
        let types = self.types(cx);
        self.diagnostics = match &self.doc {
            Some(doc) if !types.is_empty() => validate(doc, &types),
            _ => Vec::new(),
        };
        self.diagnostics
            .extend(self.ext_diagnostics.iter().cloned());
        self.schedule_check(cx);
    }

    /// Asks the extension's code to check the graph (`check-graph`), half a
    /// second after the last change.
    fn schedule_check(&mut self, cx: &mut Context<Self>) {
        let registry = crate::extensions::registry(cx);
        let wants = self.kind(&registry).is_some_and(|kind| kind.spec.check);
        let (Some(doc), Some(path), true) = (self.doc.clone(), self.path.clone(), wants) else {
            return;
        };
        let extension = carbon_core::extension::ExtensionId::new(
            self.key.split('/').next().unwrap_or_default(),
        );
        self._check = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let Ok(task) = this.update(cx, |_, cx| {
                crate::extensions::check_graph(extension, path.display().to_string(), doc, cx)
            }) else {
                return;
            };
            let found = task.await.unwrap_or_default();
            _ = this.update(cx, |this, cx| {
                this.ext_diagnostics = found;
                let types = this.types(cx);
                this.diagnostics = match &this.doc {
                    Some(doc) if !types.is_empty() => validate(doc, &types),
                    _ => Vec::new(),
                };
                this.diagnostics
                    .extend(this.ext_diagnostics.iter().cloned());
                cx.notify();
            });
        });
    }

    /// Applies an edit through the history; `merge`: consecutive edits with
    /// the same key within a second are one undo step (typing in a field).
    fn edit(&mut self, edit: GraphEdit, merge: Option<String>, cx: &mut Context<Self>) {
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let logic = edit.changes_logic();
        self.history.apply(doc, edit, merge);
        self.after_change(logic, cx);
    }

    /// After any change to the document: check it again and save soon.
    fn after_change(&mut self, logic: bool, cx: &mut Context<Self>) {
        if let Some(doc) = &self.doc {
            self.selected.retain(|id| doc.nodes.contains_key(id));
            if let Some(edge) = &self.selected_edge
                && doc.edge(edge).is_none()
            {
                self.selected_edge = None;
            }
        }
        if logic {
            self.revalidate(cx);
        }
        self.save_state = SaveState::Unsaved;
        self.schedule_save(cx);
        cx.notify();
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self._autosave = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            _ = this.update(cx, |this, cx| this.save_now(cx));
        });
    }

    /// Writes the file now (when there are changes to write).
    fn save_now(&mut self, cx: &mut Context<Self>) {
        if self.save_state == SaveState::Saved || self.disk_conflict {
            return;
        }
        let (Some(path), Some(doc)) = (self.path.clone(), self.doc.as_mut()) else {
            return;
        };
        doc.layout.viewport = self.view.viewport();
        self.save_state = match write_file(&path, doc) {
            Ok(()) => {
                self.disk_mtime = mtime(&path);
                SaveState::Saved
            }
            Err(err) => SaveState::Failed(err),
        };
        cx.notify();
    }

    /// Checks the file for changes made elsewhere: reloads when there are
    /// no unsaved edits, else asks.
    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self._poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(DISK_POLL).await;
                let alive = this.update(cx, |this, cx| {
                    let Some(path) = this.path.clone() else {
                        return;
                    };
                    let now = mtime(&path);
                    if now.is_none() || now == this.disk_mtime || this.disk_conflict {
                        return;
                    }
                    if this.save_state == SaveState::Saved {
                        this.open(path, cx);
                    } else {
                        this.disk_conflict = true;
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
    }

    /// The file changed on disk and here: keep the disk's version.
    fn reload_from_disk(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.path.clone() {
            self.open(path, cx);
        }
    }

    /// The file changed on disk and here: write this version over it.
    fn keep_mine(&mut self, cx: &mut Context<Self>) {
        self.disk_conflict = false;
        self.save_state = SaveState::Unsaved;
        self.save_now(cx);
    }

    // ---- actions -------------------------------------------------------

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(doc) = self.doc.as_mut()
            && self.history.undo(doc)
        {
            self.after_change(true, cx);
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(doc) = self.doc.as_mut()
            && self.history.redo(doc)
        {
            self.after_change(true, cx);
        }
    }

    fn delete_selection(&mut self, _: &DeleteSelection, _: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = &self.doc else {
            return;
        };
        let mut edits = Vec::new();
        if let Some(edge) = &self.selected_edge
            && let Some(edit) = GraphEdit::remove_edge(doc, edge)
        {
            edits.push(edit);
        }
        // One node at a time, each against the document as the ones before
        // it leave it (their arrows go with them).
        let mut scratch = doc.clone();
        for id in &self.selected {
            if let Some(edit) = GraphEdit::remove_node(&scratch, id) {
                edit.apply(&mut scratch);
                edits.push(edit);
            }
        }
        if edits.is_empty() {
            return;
        }
        self.selected.clear();
        self.selected_edge = None;
        self.clear_inputs();
        self.edit(GraphEdit::Batch(edits), None, cx);
    }

    fn duplicate(&mut self, _: &Duplicate, _: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = &self.doc else {
            return;
        };
        let types = self.types(cx);
        let mut scratch = doc.clone();
        let mut edits = Vec::new();
        let mut copies = BTreeSet::new();
        for id in &self.selected {
            let Some(node) = doc.nodes.get(id) else {
                continue;
            };
            let limited = types
                .iter()
                .find(|t| t.id == node.kind)
                .and_then(|t| t.max_count)
                .is_some_and(|max| {
                    scratch
                        .nodes
                        .values()
                        .filter(|n| n.kind == node.kind)
                        .count()
                        >= max
                });
            if limited {
                continue;
            }
            let copy = scratch.next_node_id(&node.kind);
            let pos = doc.pos(id);
            let edit = GraphEdit::AddNode {
                id: copy.clone(),
                node: node.clone(),
                pos: Pos::new(pos.x + 32., pos.y + 32.),
                index: usize::MAX,
            };
            edit.apply(&mut scratch);
            edits.push(edit);
            copies.insert(copy);
        }
        if edits.is_empty() {
            return;
        }
        self.edit(GraphEdit::Batch(edits), None, cx);
        self.select_nodes(copies, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(doc) = &self.doc {
            let all = doc.nodes.keys().cloned().collect();
            self.select_nodes(all, cx);
        }
    }

    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom_by(1.25, cx);
    }

    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom_by(0.8, cx);
    }

    fn zoom_fit(&mut self, _: &ZoomFit, _: &mut Window, cx: &mut Context<Self>) {
        self.fit(cx);
    }

    fn zoom_reset(&mut self, _: &ZoomReset, _: &mut Window, cx: &mut Context<Self>) {
        let size = self.bounds.get().size;
        self.view = self
            .view
            .zoom_at(1., point(size.width / 2., size.height / 2.));
        cx.notify();
    }

    fn save_graph(&mut self, _: &SaveGraph, _: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.take().is_none() {
            self.selected.clear();
            self.selected_edge = None;
            self.clear_inputs();
        }
        cx.notify();
    }

    /// A top-bar action of the graph kind (Run, Stop): the file is saved,
    /// then the extension is told, with the selected blocks.
    fn run_action(&mut self, action: String, cx: &mut Context<Self>) {
        self.save_now(cx);
        let Some(path) = self.path.clone() else {
            return;
        };
        let root = crate::workspace_ops::current_core(cx)
            .read(cx)
            .workspace()
            .map(|w| w.root().to_path_buf());
        let relative = root
            .and_then(|root| {
                let root = dunce::canonicalize(&root).unwrap_or(root);
                let file = dunce::canonicalize(&path).unwrap_or(path.clone());
                file.strip_prefix(&root).ok().map(Path::to_path_buf)
            })
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let extension = self.key.split('/').next().unwrap_or_default().to_string();
        if action == "run" {
            self.tab = InspectorTab::Run;
        }
        crate::extensions::host::deliver(
            carbon_core::extension::ExtensionId::new(extension),
            carbon_core::extension::ExtensionEvent::GraphAction {
                path: relative,
                action,
                selection: self.selected.iter().cloned().collect(),
            },
            cx,
        );
        cx.notify();
    }

    /// Zooms about the canvas centre.
    fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        let size = self.bounds.get().size;
        let center = point(size.width / 2., size.height / 2.);
        self.view = self.view.zoom_at(self.view.zoom * factor, center);
        cx.notify();
    }

    /// Shows every block.
    fn fit(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = &self.doc else {
            return;
        };
        let registry = crate::extensions::registry(cx);
        let types = self
            .kind(&registry)
            .map(|k| k.node_types)
            .unwrap_or_default();
        let boxes = geometry::boxes(doc, &types);
        let size = self.bounds.get().size;
        if size.width <= px(0.) {
            self.fit_pending = true;
            return;
        }
        self.view = View::fit(&boxes, (f32::from(size.width), f32::from(size.height)));
        cx.notify();
    }

    fn select_nodes(&mut self, nodes: BTreeSet<String>, cx: &mut Context<Self>) {
        if nodes != self.selected {
            self.clear_inputs();
        }
        self.selected = nodes;
        self.selected_edge = None;
        cx.notify();
    }

    /// Adds a block of `node_type` in the middle of what is shown.
    fn add_node(&mut self, node_type: &str, cx: &mut Context<Self>) {
        let Some(doc) = &self.doc else {
            return;
        };
        let types = self.types(cx);
        let Some(spec) = types.iter().find(|t| t.id == node_type) else {
            return;
        };
        if spec
            .max_count
            .is_some_and(|max| doc.nodes.values().filter(|n| n.kind == spec.id).count() >= max)
        {
            return;
        }
        let size = self.bounds.get().size;
        let (cx_, cy_) = self.view.to_graph(point(size.width / 2., size.height / 2.));
        let id = doc.next_node_id(&spec.id);
        let node = spec.new_node();
        // In free space: below the middle, past any block it would cover.
        let registry = crate::extensions::registry(cx);
        let node_types = self
            .kind(&registry)
            .map(|k| k.node_types)
            .unwrap_or_default();
        let boxes = geometry::boxes(doc, &node_types);
        let new_box = geometry::NodeBox::new(&id, &node, Some(spec), Pos::default());
        let mut pos = Pos::new(
            geometry::snap((cx_ - geometry::NODE_WIDTH / 2.) as f64),
            geometry::snap((cy_ - new_box.height / 2.) as f64),
        );
        for _ in 0..40 {
            let gap = 16.;
            let covers = boxes.iter().any(|b| {
                pos.x < b.pos.x + (geometry::NODE_WIDTH + gap) as f64
                    && b.pos.x < pos.x + (geometry::NODE_WIDTH + gap) as f64
                    && pos.y < b.pos.y + (b.height + gap) as f64
                    && b.pos.y < pos.y + (new_box.height + gap) as f64
            });
            if !covers {
                break;
            }
            pos.y = geometry::snap(pos.y + 48.);
        }
        self.edit(
            GraphEdit::AddNode {
                id: id.clone(),
                node,
                pos,
                index: usize::MAX,
            },
            None,
            cx,
        );
        self.select_nodes(BTreeSet::from([id]), cx);
    }

    /// Adds an arrow when the ports fit (and it is not there yet).
    fn connect(&mut self, from: PortRef, to: PortRef, cx: &mut Context<Self>) {
        let Some(doc) = &self.doc else {
            return;
        };
        if from.node == to.node && !self.is_loop_back(&to, cx) {
            return;
        }
        if doc.edges.iter().any(|e| e.from == from && e.to == to) {
            return;
        }
        // A single-arrow input takes the new arrow in place of the old one.
        let mut edits = Vec::new();
        let types = self.types(cx);
        let single = doc
            .nodes
            .get(&to.node)
            .and_then(|node| types.iter().find(|t| t.id == node.kind))
            .and_then(|spec| spec.input(&to.port))
            .is_some_and(|port| !port.multi);
        let mut scratch = doc.clone();
        if single {
            let old: Vec<String> = doc
                .edges
                .iter()
                .filter(|e| e.to == to)
                .map(|e| e.id.clone())
                .collect();
            for id in old {
                if let Some(edit) = GraphEdit::remove_edge(&scratch, &id) {
                    edit.apply(&mut scratch);
                    edits.push(edit);
                }
            }
        }
        let edge = carbon_core::graph::Edge {
            id: scratch.next_edge_id(),
            from,
            to,
        };
        edits.push(GraphEdit::AddEdge {
            index: scratch.edges.len(),
            edge,
        });
        self.edit(GraphEdit::Batch(edits), None, cx);
    }

    fn is_loop_back(&self, port: &PortRef, cx: &App) -> bool {
        let Some(doc) = &self.doc else {
            return false;
        };
        let types = self.types(cx);
        doc.nodes
            .get(&port.node)
            .and_then(|node| types.iter().find(|t| t.id == node.kind))
            .and_then(|spec| spec.input(&port.port))
            .is_some_and(|p| p.loop_back)
    }

    /// The problems of `node` (or of the graph with `None`), worst first.
    fn problems_of(&self, node: Option<&str>) -> Vec<&Diagnostic> {
        let mut found: Vec<_> = self
            .diagnostics
            .iter()
            .filter(|d| d.node.as_deref() == node && (node.is_some() || d.edge.is_none()))
            .collect();
        found.sort_by_key(|d| std::cmp::Reverse(d.severity));
        found
    }

    fn edge_problems(&self, edge: &str) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.edge.as_deref() == Some(edge))
            .collect()
    }

    // ---- drawing -------------------------------------------------------

    fn render_toolbar(&self, kind: &GraphKind, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let doc = self.doc.as_ref();
        let name = doc.map(|d| d.name.clone()).unwrap_or_default();
        let (state_text, state_color) = match &self.save_state {
            SaveState::Saved => ("Saved".to_string(), muted),
            SaveState::Unsaved => ("Unsaved".to_string(), muted),
            SaveState::Failed(err) => (format!("Save failed: {err}"), theme.danger),
        };
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        let warnings = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count();
        let mut categories: Vec<(String, Vec<(String, String)>)> = Vec::new();
        for spec in &kind.node_types {
            let category = spec.category.clone().unwrap_or_else(|| "Blocks".into());
            let entry = (spec.id.clone(), spec.title.clone());
            match categories.iter_mut().find(|(c, _)| *c == category) {
                Some((_, list)) => list.push(entry),
                None => categories.push((category, vec![entry])),
            }
        }
        let entity = cx.entity();
        let tool = |id: &'static str, icon: IconName, tip: &'static str| {
            Button::new(id).icon(icon).ghost().small().tooltip(tip)
        };
        h_flex()
            .flex_none()
            .w_full()
            .gap_1()
            .px(fixed::GRAPH_TOOLBAR_PADDING)
            .py_1()
            .border_b_1()
            .border_color(border)
            .child(
                Button::new("graph-files")
                    .icon(IconName::ChevronLeft)
                    .ghost()
                    .small()
                    .tooltip("All files")
                    .on_click(cx.listener(|this, _, _, cx| this.close_file(cx))),
            )
            .child(
                Button::new("graph-add")
                    .label("Add")
                    .icon(IconName::Plus)
                    .small()
                    .outline()
                    .dropdown_caret(true)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        let many = categories.len() > 1;
                        for (category, items) in &categories {
                            if many {
                                menu = menu.label(category.clone());
                            }
                            for (id, title) in items {
                                let (entity, id) = (entity.clone(), id.clone());
                                menu = menu.item(PopupMenuItem::new(title.clone()).on_click(
                                    move |_, _, cx| {
                                        entity.update(cx, |this, cx| this.add_node(&id, cx))
                                    },
                                ));
                            }
                        }
                        menu
                    }),
            )
            .child(
                tool("graph-undo", IconName::Undo2, "Undo (Ctrl+Z)")
                    .disabled(!self.history.can_undo())
                    .on_click(cx.listener(|this, _, w, cx| this.undo(&Undo, w, cx))),
            )
            .child(
                tool("graph-redo", IconName::Redo2, "Redo (Ctrl+Y)")
                    .disabled(!self.history.can_redo())
                    .on_click(cx.listener(|this, _, w, cx| this.redo(&Redo, w, cx))),
            )
            .child(
                tool("graph-zoom-out", IconName::Minus, "Zoom out (-)")
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_by(0.8, cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .min_w(fixed::GRAPH_ZOOM_LABEL_WIDTH)
                    .text_center()
                    .child(format!("{:.0}%", self.view.zoom * 100.)),
            )
            .child(
                tool("graph-zoom-in", IconName::Plus, "Zoom in (+)")
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_by(1.25, cx))),
            )
            .child(
                tool("graph-fit", IconName::Maximize, "Show all (0)")
                    .on_click(cx.listener(|this, _, _, cx| this.fit(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_weight(FontWeight::MEDIUM)
                    .child(name),
            )
            .when(errors + warnings > 0, |bar| {
                let text = match (errors, warnings) {
                    (0, w) => format!("{w} warning{}", if w == 1 { "" } else { "s" }),
                    (e, 0) => format!("{e} problem{}", if e == 1 { "" } else { "s" }),
                    (e, w) => format!("{e} problems, {w} warnings"),
                };
                bar.child(
                    div()
                        .text_xs()
                        .text_color(if errors > 0 {
                            cx.theme().danger
                        } else {
                            cx.theme().warning
                        })
                        .child(text),
                )
            })
            .child(div().text_xs().text_color(state_color).child(state_text))
            .children(self.render_actions(kind, cx))
    }

    /// The graph kind's actions, by whether a run is going.
    fn render_actions(&self, kind: &GraphKind, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let running = self.run.as_ref().is_some_and(|run| run.state.is_running());
        let active = crate::extensions::registry(cx).is_active(&kind.loaded.def.id);
        kind.spec
            .actions
            .iter()
            .filter(|action| match action.when.as_deref() {
                Some("idle") => !running,
                Some("running") => running,
                _ => true,
            })
            .map(|action| {
                let id = action.id.clone();
                let mut button = Button::new(SharedString::from(format!("graph-action-{id}")))
                    .label(action.title.clone())
                    .small()
                    .disabled(!active);
                if let Some(icon) = crate::extension_panel::named_icon(action.icon.as_deref()) {
                    button = button.icon(icon);
                }
                if action.primary {
                    button = button.primary();
                }
                button
                    .on_click(cx.listener(move |this, _, _, cx| this.run_action(id.clone(), cx)))
                    .into_any_element()
            })
            .collect()
    }

    fn render_conflict(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .flex_none()
            .w_full()
            .gap_2()
            .px(fixed::GRAPH_TOOLBAR_PADDING)
            .py_1()
            .bg(cx.theme().warning.opacity(0.15))
            .text_sm()
            .child(
                div()
                    .flex_1()
                    .child("This file changed on disk, and you have unsaved changes."),
            )
            .child(
                Button::new("graph-reload")
                    .label("Reload")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.reload_from_disk(cx))),
            )
            .child(
                Button::new("graph-keep")
                    .label("Keep Mine")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.keep_mine(cx))),
            )
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A centered note filling the panel.
fn message(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .p(fixed::PANEL_PADDING)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
        .into_any_element()
}

impl EventEmitter<PanelEvent> for GraphPanel {}

impl Focusable for GraphPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for GraphPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        state.info = PanelInfo::Panel(serde_json::json!({
            "key": self.key,
            "title": self.title.as_ref(),
            "path": self.path.as_ref().map(|p| p.display().to_string()),
        }));
        state
    }

    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
    }
}

impl Panel for GraphPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        let name = self
            .doc
            .as_ref()
            .map(|doc| doc.name.clone())
            .filter(|name| !name.trim().is_empty())
            .or_else(|| {
                self.path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
            });
        Some(name.map(Into::into).unwrap_or_else(|| self.title.clone()))
    }

    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tab_name(cx).unwrap_or_default()
    }

    fn always_tab_bar(&self, _: &App) -> bool {
        true
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.save_now(cx);
        crate::workbench::close_panel(cx.entity(), window, cx);
        true
    }
}

impl Render for GraphPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = crate::extensions::registry(cx);
        let extension = self.key.split('/').next().unwrap_or_default().to_string();
        let base = v_flex()
            .id("graph-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save_graph));
        let Some(kind) = self.kind(&registry) else {
            let text = if registry.panel(&self.key).is_none() {
                format!(
                    "This panel's extension ({extension}) is not installed. Install it in Manage Extensions, or close the tab."
                )
            } else {
                format!("The extension {extension} is turned off. Turn it on in Manage Extensions.")
            };
            return base.child(message(text, cx));
        };
        let Some(_) = &self.path else {
            return base.child(files::render_files(self, &kind, cx));
        };
        if let Some(err) = &self.load_error {
            return base.child(message(format!("This file cannot be opened: {err}"), cx));
        }
        self.run = self
            .path
            .as_deref()
            .and_then(|path| crate::extensions::host::latest_run(path, cx));
        if self.fit_pending && self.bounds.get().size.width > px(0.) {
            self.fit_pending = false;
            self.fit(cx);
        }
        let toolbar = self.render_toolbar(&kind, cx).into_any_element();
        let conflict = self
            .disk_conflict
            .then(|| self.render_conflict(cx).into_any_element());
        let canvas = canvas::render_canvas(self, &kind, window, cx);
        let inspector = inspector::render_inspector(self, &kind, window, cx);
        let border = cx.theme().border;
        base.child(toolbar).children(conflict).child(
            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(div().flex_1().min_w_0().h_full().child(canvas))
                .child(
                    div()
                        .flex_none()
                        .w(fixed::GRAPH_INSPECTOR_WIDTH)
                        .h_full()
                        .border_l_1()
                        .border_color(border)
                        .child(inspector),
                ),
        )
    }
}
