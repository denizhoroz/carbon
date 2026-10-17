//! Editor tabs: one panel per open buffer, drawn by the kit code editor.
//!
//! The core buffer is the source of truth. The kit editor edits its own copy
//! of the text; after each change the buffer takes that rope (the same rope
//! type, so a cheap clone). When the core changes a buffer itself (a reload
//! from disk), the panel sees the revision move and puts the new text into the
//! editor. The panel is the only place that knows about the kit editor, so the
//! view can be replaced without touching the core.

mod language;
mod lsp;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use carbon_core::lsp::{self as core_lsp, lsp_types};
use carbon_core::{BufferId, Core, CoreError};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{
    BasePanel, Panel, PanelEvent, PanelInfo, PanelState, panel_handle, register_panel,
};
use gpui_kit::component::input::{Editor, EditorState, GoToDefinition, InputEvent, RopeExt as _};
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::{ActiveTheme, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::style::fixed;
use crate::workspace_ops::current_core;

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "Editor";
/// Quiet time after typing before the language server gets the new text.
const LSP_CHANGE_DELAY: Duration = Duration::from_millis(150);

actions!(editor, [SaveFile, CloseEditor]);

pub struct EditorPanel {
    core: Entity<Core>,
    focus_handle: FocusHandle,
    doc: Option<Doc>,
    /// "Don't Save" was chosen: drop the unsaved text when the tab goes.
    discard: bool,
    _core_sub: Subscription,
}

/// The open buffer and the kit editor showing it.
struct Doc {
    buffer: BufferId,
    state: Entity<EditorState>,
    /// The buffer revision the editor's text matches.
    synced: u64,
    /// A file outside the folder: shown read-only.
    readonly: bool,
    /// Generation of the language server diagnostics shown.
    diagnostics: u64,
    /// Sends the text to the language server once typing pauses.
    lsp_change: Task<()>,
    _events: Subscription,
}

impl EditorPanel {
    /// An empty editor tab (no file yet).
    pub fn new(cx: &mut App) -> Entity<Self> {
        let core = current_core(cx);
        cx.new(|cx| Self {
            _core_sub: cx.observe(&core, |_, _, cx| cx.notify()),
            core,
            focus_handle: cx.focus_handle(),
            doc: None,
            discard: false,
        })
    }

    /// A tab showing buffer `id`.
    pub fn for_buffer(id: BufferId, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let panel = Self::new(cx);
        panel.update(cx, |panel, cx| panel.set_buffer(id, window, cx));
        panel
    }

    /// Rebuilds a tab from a saved layout: opens its file again (or finds the
    /// buffer still open, with its unsaved text).
    fn restore(path: Option<PathBuf>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let panel = Self::new(cx);
        if let Some(path) = path {
            let core = current_core(cx);
            match core.update(cx, |core, _| core.open_file(&path)) {
                Ok(id) => panel.update(cx, |panel, cx| panel.set_buffer(id, window, cx)),
                // Saved layouts are not per folder (yet), so files of another
                // folder are common: drop their tabs instead of showing errors.
                Err(err) => {
                    eprintln!("not restoring editor tab {}: {err}", path.display());
                    let stale = panel.clone();
                    window.defer(cx, move |window, cx| {
                        crate::workbench::close_panel(stale, window, cx)
                    });
                }
            }
        }
        panel
    }

    /// The buffer this tab shows, if any.
    pub fn buffer(&self) -> Option<BufferId> {
        self.doc.as_ref().map(|doc| doc.buffer)
    }

    /// Shows buffer `id` in this tab.
    pub fn set_buffer(&mut self, id: BufferId, window: &mut Window, cx: &mut Context<Self>) {
        let core = self.core.read(cx);
        let Some(buffer) = core.buffers.get(id) else {
            return;
        };
        let text = buffer.text();
        let revision = buffer.revision();
        let path = buffer.path().map(Path::to_path_buf);
        let language = path
            .as_deref()
            .map(language::for_path)
            .unwrap_or(language::PLAIN);
        // Files outside the folder (a library a definition led to) are for
        // reading only.
        let readonly = match (&path, core.workspace()) {
            (Some(path), Some(workspace)) => !path.starts_with(workspace.root()),
            _ => false,
        };
        // No code folding for now: the kit's folding delays the redraw after
        // each keystroke (~25 ms), takes seconds to open a large file, and
        // walks the syntax tree recursively on a background thread, which a
        // broken parse of a big file overflows (measured 2026-10-17).
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(language)
                .soft_wrap(false)
                .folding(false)
        });
        state.update(cx, |state, cx| state.set_value(text, window, cx));
        if let Some(path) = path.as_deref()
            && core_lsp::server_config(path).is_some()
        {
            let hooks = lsp::LspHooks::new(self.core.clone(), id);
            let uri = core_lsp::path_to_uri(path);
            state.update(cx, |state, _| {
                let lsp = state.lsp_mut();
                lsp.completion_provider = Some(hooks.clone());
                lsp.hover_provider = Some(hooks.clone());
                lsp.definition_provider = Some(hooks);
                lsp.show_document = Some(Rc::new(move |params, window, cx| {
                    show_document(&uri, params, window, cx)
                }));
            });
            crate::lsp_ops::ensure_started(&self.core, path, cx);
        }
        let _events = cx.subscribe_in(&state, window, Self::on_editor_event);
        self.doc = Some(Doc {
            buffer: id,
            state,
            synced: revision,
            readonly,
            diagnostics: 0,
            lsp_change: Task::ready(()),
            _events,
        });
        cx.notify();
    }

    /// Moves the cursor to an LSP position (UTF-16 columns).
    pub fn go_to(&self, position: lsp_types::Position, window: &mut Window, cx: &mut App) {
        let Some(doc) = &self.doc else {
            return;
        };
        doc.state.update(cx, |state, cx| {
            let offset = core_lsp::position_to_offset(state.text(), position);
            let position = state.text().offset_to_position(offset);
            state.set_cursor_position(position, window, cx);
        });
    }

    pub fn focus_editor(&self, window: &mut Window, cx: &mut App) {
        match &self.doc {
            Some(doc) => doc.state.focus_handle(cx).focus(window, cx),
            None => self.focus_handle.focus(window, cx),
        }
    }

    fn on_editor_event(
        &mut self,
        state: &Entity<EditorState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        match event {
            InputEvent::Change => {
                let rope = state.read(cx).text().clone();
                let id = doc.buffer;
                let synced = self.core.update(cx, |core, cx| {
                    let buffer = core.buffers.get_mut(id)?;
                    let was_dirty = buffer.is_dirty();
                    buffer.set_rope(rope);
                    // Others care when the file turns dirty, not about each key.
                    if !was_dirty {
                        cx.notify();
                    }
                    Some(buffer.revision())
                });
                if let Some(synced) = synced {
                    doc.synced = synced;
                }
                let core = self.core.downgrade();
                doc.lsp_change = cx.spawn(async move |_, cx| {
                    cx.background_executor().timer(LSP_CHANGE_DELAY).await;
                    _ = core.update(cx, |core, _| core.lsp_change(id));
                });
                cx.notify();
            }
            InputEvent::Focus => {
                let id = doc.buffer;
                self.core.update(cx, |core, _| core.set_active_buffer(id));
            }
            _ => {}
        }
    }

    /// Puts text the core changed (a reload) into the editor, keeping the
    /// cursor where it was; drops the editor when the buffer was closed.
    fn sync_from_core(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.doc.as_mut() else {
            return;
        };
        let core = self.core.read(cx);
        let Some(buffer) = core.buffers.get(doc.buffer) else {
            self.doc = None;
            return;
        };
        if buffer.revision() != doc.synced {
            let text = buffer.text();
            doc.synced = buffer.revision();
            doc.state.update(cx, |state, cx| {
                let cursor = state.cursor_position();
                state.replace_all(text, window, cx);
                state.set_cursor_position(cursor, window, cx);
            });
        }
        // Diagnostics the server published since the last look.
        let core = self.core.read(cx);
        let diagnostics = core
            .buffers
            .get(doc.buffer)
            .and_then(|buffer| buffer.path())
            .and_then(|path| core.lsp.diagnostics(path))
            .filter(|diagnostics| diagnostics.generation != doc.diagnostics)
            .cloned();
        if let Some(diagnostics) = diagnostics {
            doc.diagnostics = diagnostics.generation;
            doc.state.update(cx, |state, cx| {
                let text = state.text().clone();
                // The kit counts columns in characters, the server in UTF-16.
                let items = diagnostics.items.into_iter().map(|mut item| {
                    for position in [&mut item.range.start, &mut item.range.end] {
                        let offset = core_lsp::position_to_offset(&text, *position);
                        *position = text.offset_to_position(offset);
                    }
                    item
                });
                if let Some(set) = state.diagnostics_mut() {
                    set.reset(&text);
                    set.extend(items);
                }
                cx.notify();
            });
        }
    }

    fn path(&self, cx: &App) -> Option<PathBuf> {
        let doc = self.doc.as_ref()?;
        let core = self.core.read(cx);
        core.buffers.get(doc.buffer)?.path().map(Path::to_path_buf)
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.doc.as_ref().is_some_and(|doc| {
            self.core
                .read(cx)
                .buffers
                .get(doc.buffer)
                .is_some_and(|buffer| buffer.is_dirty())
        })
    }

    fn name(&self, cx: &App) -> SharedString {
        match (&self.doc, self.path(cx)) {
            (None, _) => "Editor".into(),
            (Some(_), Some(path)) => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
                .into(),
            (Some(_), None) => "Untitled".into(),
        }
    }

    pub fn save(&mut self, _: &SaveFile, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = self.save_now(cx) {
            window.push_notification(format!("Cannot save: {err}"), cx);
        }
    }

    fn save_now(&mut self, cx: &mut Context<Self>) -> carbon_core::Result<()> {
        let Some(id) = self.buffer() else {
            return Ok(());
        };
        let result = self.core.update(cx, |core, cx| {
            let result = core.save_buffer(id);
            cx.notify();
            result
        });
        cx.notify();
        result
    }

    /// Ctrl+W: closes the tab, asking first when the file has unsaved changes.
    pub fn close(&mut self, _: &CloseEditor, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.entity();
        if !self.is_dirty(cx) {
            crate::workbench::close_panel(panel, window, cx);
            return;
        }
        let name = self.name(cx);
        let save_panel = panel.clone();
        let save = move |window: &mut Window, cx: &mut App| {
            let saved = save_panel.update(cx, |panel, cx| panel.save_now(cx));
            match saved {
                Ok(()) => crate::workbench::close_panel(save_panel.clone(), window, cx),
                Err(err) => window.push_notification(format!("Cannot save: {err}"), cx),
            }
        };
        let save = std::rc::Rc::new(save);
        window.open_dialog(cx, move |dialog, _, cx| {
            let on_ok = save.clone();
            let on_save = save.clone();
            let discard_panel = panel.clone();
            dialog
                .title("Unsaved changes")
                .w(fixed::DIALOG_WIDTH)
                .on_ok(move |_, window, cx| {
                    on_ok(window, cx);
                    true
                })
                .child(
                    v_flex()
                        .gap(fixed::PANEL_GAP)
                        .child(format!("Save the changes to {name}?"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Your changes are lost if you don't save them."),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("editor-close-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("editor-close-discard")
                                .label("Don't Save")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    discard_panel.update(cx, |panel, _| panel.discard = true);
                                    crate::workbench::close_panel(
                                        discard_panel.clone(),
                                        window,
                                        cx,
                                    );
                                }),
                        )
                        .child(
                            Button::new("editor-close-save")
                                .primary()
                                .label("Save")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    on_save(window, cx);
                                }),
                        ),
                )
        });
    }
}

impl EditorPanel {
    /// Bar over the editor when the file changed on disk under unsaved edits.
    fn render_conflict(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .flex_none()
            .px(fixed::TREE_PADDING)
            .py(fixed::TREE_ITEM_GAP)
            .gap(fixed::PANEL_GAP)
            .border_b_1()
            .border_color(theme.border)
            .text_sm()
            .child(
                div()
                    .flex_1()
                    .text_color(theme.warning)
                    .child("This file changed on disk. You have unsaved changes."),
            )
            .child(
                Button::new("editor-conflict-reload")
                    .small()
                    .label("Reload")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.resolve_conflict(true, window, cx)),
                    ),
            )
            .child(
                Button::new("editor-conflict-keep")
                    .small()
                    .label("Keep Mine")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.resolve_conflict(false, window, cx)),
                    ),
            )
    }

    fn resolve_conflict(&mut self, reload: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.buffer() else {
            return;
        };
        let result = self.core.update(cx, |core, cx| {
            let buffer = core
                .buffers
                .get_mut(id)
                .ok_or(CoreError::NotFound("buffer"))?;
            let result = if reload {
                buffer.reload()
            } else {
                buffer.ignore_disk_change();
                Ok(())
            };
            cx.notify();
            result
        });
        if let Err(err) = result {
            window.push_notification(format!("Cannot reload: {err}"), cx);
        }
    }
}

/// Go to definition into another file: opens it in its tab (a library file
/// outside the workspace too) and moves to the target. False for the file
/// itself, which the kit editor handles.
fn show_document(
    own: &lsp_types::Uri,
    params: &lsp_types::ShowDocumentParams,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if &params.uri == own {
        return false;
    }
    let Some(path) = core_lsp::uri_to_path(&params.uri) else {
        return false;
    };
    let selection = params.selection.map(|range| range.start);
    // Deferred: this runs inside the source editor's update.
    window.defer(cx, move |window, cx| {
        let core = current_core(cx);
        let opened = core.update(cx, |core, cx| {
            let result = core.open_file(&path).or_else(|_| core.open_any_file(&path));
            cx.notify();
            result
        });
        let id = match opened {
            Ok(id) => id,
            Err(err) => {
                window.push_notification(format!("Cannot open {}: {err}", path.display()), cx);
                return;
            }
        };
        crate::workbench::open_buffer(id, window, cx);
        if let (Some(position), Some(editor)) = (selection, crate::workbench::active_editor(cx)) {
            editor.update(cx, |editor, cx| editor.go_to(position, window, cx));
        }
    });
    true
}

/// Lets saved layouts recreate editor tabs, and binds the editor keys.
pub fn register(cx: &mut App) {
    // App-wide: outside an editor they act on the active one (the app root
    // forwards them), like VS Code.
    cx.bind_keys([
        KeyBinding::new("secondary-s", SaveFile, None),
        KeyBinding::new("secondary-w", CloseEditor, None),
        // The kit offers it only in the right-click menu and on Ctrl+click.
        KeyBinding::new("f12", GoToDefinition, Some("Input")),
    ]);
    register_panel(cx, PANEL_NAME, |context, window, cx| {
        let path = match context.info() {
            PanelInfo::Panel(info) => info.get("path").and_then(|p| p.as_str()).map(PathBuf::from),
            _ => None,
        };
        panel_handle(EditorPanel::restore(path, window, cx))
    });
}

impl EventEmitter<PanelEvent> for EditorPanel {}

impl Focusable for EditorPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.doc {
            Some(doc) => doc.state.focus_handle(cx),
            None => self.focus_handle.clone(),
        }
    }
}

impl BasePanel for EditorPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    /// A tab with unsaved changes has no close button; Ctrl+W asks first.
    fn closable(&self, cx: &App) -> bool {
        !self.is_dirty(cx)
    }

    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active && let Some(id) = self.buffer() {
            self.core.update(cx, |core, _| core.set_active_buffer(id));
        }
    }

    /// Closes the buffer with the tab. One with unsaved changes stays open
    /// (a layout switch removes tabs too) unless "Don't Save" was chosen.
    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.buffer() else {
            return;
        };
        let force = self.discard;
        self.core.update(cx, |core, cx| {
            if core.close_buffer(id, force).is_ok() {
                cx.notify();
            }
        });
    }

    fn dump(&self, cx: &App) -> PanelState {
        let mut state = PanelState::new(PANEL_NAME);
        if let Some(path) = self.path(cx) {
            state.info = PanelInfo::panel(serde_json::json!({ "path": path }));
        }
        state
    }
}

impl Panel for EditorPanel {
    /// None: the tab strip then draws `title`, which has the unsaved dot.
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        None
    }

    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dirty = self.is_dirty(cx);
        h_flex()
            .gap(fixed::TREE_ITEM_GAP)
            .child(self.name(cx))
            .when(dirty, |this| {
                this.child(
                    div()
                        .flex_none()
                        .size(fixed::TREE_DOT_SIZE)
                        .rounded_full()
                        .bg(cx.theme().foreground),
                )
            })
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> PopupMenu {
        if self.doc.is_none() {
            return menu;
        }
        menu.menu("Save", Box::new(SaveFile))
            .menu("Close", Box::new(CloseEditor))
    }
}

impl Render for EditorPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_from_core(window, cx);
        let muted = cx.theme().muted_foreground;
        let base = v_flex()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::close))
            .size_full();

        if let Some(doc) = &self.doc {
            let conflict = self
                .core
                .read(cx)
                .buffers
                .get(doc.buffer)
                .is_some_and(|buffer| buffer.has_disk_conflict());
            return base
                .when(conflict, |this| this.child(self.render_conflict(cx)))
                .child(
                    div().flex_1().min_h_0().child(
                        Editor::new(&doc.state)
                            .bordered(false)
                            .readonly(doc.readonly)
                            .h_full(),
                    ),
                );
        }
        base.p(fixed::PANEL_PADDING)
            .gap(fixed::PANEL_GAP)
            .items_center()
            .justify_center()
            .text_center()
            .child("Editor")
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Pick a file in the Explorer."),
            )
    }
}
