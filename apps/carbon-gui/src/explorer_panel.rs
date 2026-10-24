//! Explorer panel. Empty state: Open Folder / Create Folder (DESIGN.md s4:
//! "no workspace = Open and Create buttons"). With a folder open: the file
//! tree, a flat virtualized row list over `carbon_core::FileTree` (folders load
//! when they expand; the core's watcher keeps loaded folders current).
//!
//! Submodules: `rows` (row drawing, markers, drag and drop), `filter`
//! (type to filter), `file_ops` (new, rename, delete, copy / cut / paste,
//! undo, copy path, reveal).

mod file_ops;
mod filter;
mod rows;

use std::path::{Path, PathBuf};

use carbon_core::{Core, EntryKind, Row};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, panel_handle, register_panel};
use gpui_kit::component::input::{self as input, Input};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu};
use gpui_kit::component::{
    ActiveTheme, Sizable, WindowExt, h_flex, scroll::ScrollableElement, v_flex,
};
use gpui_kit::*;

use crate::menus::{CreateFolder, OpenFolder, ToggleSectionSide};
use crate::style::fixed;
use crate::workspace_ops::current_core;
pub(crate) use file_ops::REVEAL_LABEL;
use file_ops::{Edit, EditKind, FileClipboard};
use filter::Filter;

/// Persistence key in saved layouts. Never change it.
pub const PANEL_NAME: &str = "Explorer";
const KEY_CONTEXT: &str = "Explorer";

/// Ask before Delete moves entries to the Recycle Bin / Trash.
const CONFIRM_DELETE: &str = "explorer.confirm_delete";

/// The Explorer's settings.
pub fn settings() -> Vec<carbon_core::settings::Setting> {
    use carbon_core::settings::{Kind, Setting};
    vec![
        Setting::new(
            CONFIRM_DELETE,
            &["Explorer"],
            "Confirm Delete",
            Kind::Bool,
            true,
        )
        .description(&format!(
            "Ask before deleting files and folders. They go to the {} either way.",
            file_ops::TRASH_LABEL
        ))
        .keywords(&["trash", "recycle bin", "remove"]),
    ]
}

actions!(
    explorer,
    [
        SelectPrev,
        SelectNext,
        ExtendPrev,
        ExtendNext,
        SelectAll,
        ExpandOrNext,
        CollapseOrParent,
        OpenSelected,
        CollapseAll,
        NewFile,
        NewFolder,
        Rename,
        Delete,
        CopyFiles,
        CutFiles,
        PasteFiles,
        UndoFileOp,
        CopyPath,
        CopyRelativePath,
        RevealInOs,
        CancelEdit
    ]
);

pub struct ExplorerPanel {
    core: Entity<Core>,
    focus_handle: FocusHandle,
    /// Visible rows (after the filter), rebuilt when the core or filter changes.
    rows: Vec<Row>,
    /// The cursor row: keyboard focus, rename target. Kept by path, so it
    /// survives rows moving after a refresh.
    selected: Option<PathBuf>,
    /// Selected rows (multi-select); operations act on these.
    marked: Vec<PathBuf>,
    /// Where a Shift range starts.
    anchor: Option<PathBuf>,
    scroll: UniformListScrollHandle,
    /// The inline name box (new entry or rename), if open.
    edit: Option<Edit>,
    /// Set by a row's right-click so the list's handler keeps that selection.
    row_right_clicked: bool,
    /// Files copied or cut in the Explorer, waiting for Paste.
    clipboard: Option<FileClipboard>,
    /// Type-to-filter box, if open.
    filter: Option<Filter>,
    _core_sub: Subscription,
}

impl ExplorerPanel {
    pub fn new(cx: &mut App) -> Entity<Self> {
        let core = current_core(cx);
        cx.new(|cx| {
            let _core_sub = cx.observe(&core, |this: &mut Self, _, cx| {
                this.rebuild_rows(cx);
                cx.notify();
            });
            let mut this = Self {
                core,
                focus_handle: cx.focus_handle(),
                rows: Vec::new(),
                selected: None,
                marked: Vec::new(),
                anchor: None,
                scroll: UniformListScrollHandle::new(),
                edit: None,
                row_right_clicked: false,
                clipboard: None,
                filter: None,
                _core_sub,
            };
            this.rebuild_rows(cx);
            this
        })
    }

    fn rebuild_rows(&mut self, cx: &App) {
        let rows = self
            .core
            .read(cx)
            .tree()
            .map(|tree| tree.visible_rows())
            .unwrap_or_default();
        self.rows = match self.filter.as_ref().map(|filter| filter.query(cx)) {
            Some(query) if !query.is_empty() => filter::filter_rows(rows, &query),
            _ => rows,
        };
        let shown = |path: &PathBuf| self.rows.iter().any(|row| &row.entry.path == path);
        if self.selected.as_ref().is_some_and(|path| !shown(path)) {
            self.selected = None;
        }
        if self.anchor.as_ref().is_some_and(|path| !shown(path)) {
            self.anchor = None;
        }
        let marked = std::mem::take(&mut self.marked);
        self.marked = marked.into_iter().filter(|path| shown(path)).collect();
    }

    fn row_ix(&self, path: &Path) -> Option<usize> {
        self.rows.iter().position(|row| row.entry.path == path)
    }

    fn selected_ix(&self) -> Option<usize> {
        self.row_ix(self.selected.as_deref()?)
    }

    fn is_marked(&self, path: &Path) -> bool {
        self.marked.iter().any(|marked| marked == path)
    }

    /// The selected paths in row order (the cursor row if nothing is marked).
    fn selection(&self) -> Vec<PathBuf> {
        let marked: Vec<PathBuf> = self
            .rows
            .iter()
            .filter(|row| self.is_marked(&row.entry.path))
            .map(|row| row.entry.path.clone())
            .collect();
        if marked.is_empty() {
            self.selected.iter().cloned().collect()
        } else {
            marked
        }
    }

    /// Plain selection of one row (click, arrow keys).
    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get(ix) {
            let path = row.entry.path.clone();
            self.marked = vec![path.clone()];
            self.anchor = Some(path.clone());
            self.move_cursor(path, ix, cx);
        }
    }

    /// Selects the given paths (after a paste, drop or create).
    fn select_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.rebuild_rows(cx);
        let Some(first) = paths.first().cloned() else {
            return;
        };
        self.marked = paths;
        self.anchor = Some(first.clone());
        if let Some(ix) = self.row_ix(&first) {
            self.move_cursor(first, ix, cx);
        } else {
            self.selected = Some(first);
            cx.notify();
        }
    }

    fn move_cursor(&mut self, path: PathBuf, ix: usize, cx: &mut Context<Self>) {
        self.selected = Some(path);
        let list_ix = match self.new_slot(cx) {
            Some((slot, _)) if ix >= slot => ix + 1,
            _ => ix,
        };
        self.scroll.scroll_to_item(list_ix, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Ctrl+click (Cmd on macOS): add or remove one row.
    fn toggle_mark(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(path) = self.rows.get(ix).map(|row| row.entry.path.clone()) else {
            return;
        };
        if self.is_marked(&path) {
            self.marked.retain(|marked| *marked != path);
        } else {
            self.marked.push(path.clone());
        }
        self.anchor = Some(path.clone());
        self.move_cursor(path, ix, cx);
    }

    /// Shift+click / Shift+arrow: everything from the anchor to `ix`.
    fn select_range(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(path) = self.rows.get(ix).map(|row| row.entry.path.clone()) else {
            return;
        };
        let anchor = self
            .anchor
            .as_deref()
            .and_then(|anchor| self.row_ix(anchor))
            .unwrap_or(ix);
        let (from, to) = (anchor.min(ix), anchor.max(ix));
        self.marked = self.rows[from..=to]
            .iter()
            .map(|row| row.entry.path.clone())
            .collect();
        if self.anchor.is_none() {
            self.anchor = Some(path.clone());
        }
        self.move_cursor(path, ix, cx);
    }

    /// Runs `f` on the core's tree; the core notifies, which rebuilds the rows.
    fn update_tree(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut carbon_core::FileTree) -> carbon_core::Result<()>,
    ) {
        let result = self.core.update(cx, |core, cx| {
            let result = core.tree_mut().map(f).unwrap_or(Ok(()));
            cx.notify();
            result
        });
        if let Err(err) = result {
            window.push_notification(format!("Cannot open folder: {err}"), cx);
        }
    }

    fn toggle(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.update_tree(window, cx, |tree| tree.toggle(&dir));
    }

    fn open_file(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if crate::graph_panel::open_graph_file(path, window, cx) {
            return;
        }
        let result = self.core.update(cx, |core, cx| {
            let result = core.open_file(path);
            cx.notify();
            result
        });
        match result {
            Ok(id) => crate::workbench::open_buffer(id, window, cx),
            Err(err) => window.push_notification(format!("Cannot open file: {err}"), cx),
        }
    }

    /// Click: folders toggle, files open (no preview tabs yet). With Ctrl
    /// (Cmd) or Shift it only changes the selection.
    fn click_row(
        &mut self,
        ix: usize,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        if modifiers.secondary() {
            return self.toggle_mark(ix, cx);
        }
        if modifiers.shift {
            return self.select_range(ix, cx);
        }
        let Some(row) = self.rows.get(ix) else {
            return;
        };
        let (path, kind) = (row.entry.path.clone(), row.entry.kind);
        self.select(ix, cx);
        match kind {
            EntryKind::Dir => self.toggle(path, window, cx),
            EntryKind::File | EntryKind::Symlink => self.open_file(&path, window, cx),
        }
    }

    fn step(&self, delta: isize) -> usize {
        let last = self.rows.len().saturating_sub(1);
        match self.selected_ix() {
            Some(ix) => ix.saturating_add_signed(delta).min(last),
            None => 0,
        }
    }

    fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_none() {
            self.select(self.step(-1), cx);
        }
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_none() {
            self.select(self.step(1), cx);
        }
    }

    fn extend_prev(&mut self, _: &ExtendPrev, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_none() {
            self.select_range(self.step(-1), cx);
        }
    }

    fn extend_next(&mut self, _: &ExtendNext, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_none() {
            self.select_range(self.step(1), cx);
        }
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_none() {
            self.marked = self.rows.iter().map(|row| row.entry.path.clone()).collect();
            cx.notify();
        }
    }

    /// Right: open a closed folder, else step into it (or down).
    fn expand_or_next(&mut self, _: &ExpandOrNext, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            return;
        }
        let Some(ix) = self.selected_ix() else {
            return self.select(0, cx);
        };
        let entry = &self.rows[ix].entry;
        if entry.kind == EntryKind::Dir && !self.is_expanded(&entry.path, cx) {
            let dir = entry.path.clone();
            self.update_tree(window, cx, |tree| tree.expand(&dir));
        } else if entry.kind == EntryKind::Dir {
            self.select(ix + 1, cx);
        }
    }

    /// Left: close an open folder, else jump to the nearest folder row above it
    /// (with compact folders the direct parent may share that row).
    fn collapse_or_parent(
        &mut self,
        _: &CollapseOrParent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.is_some() {
            return;
        }
        let Some(ix) = self.selected_ix() else {
            return;
        };
        let entry = &self.rows[ix].entry;
        if entry.kind == EntryKind::Dir && self.is_expanded(&entry.path, cx) {
            let dir = entry.path.clone();
            self.update_tree(window, cx, |tree| {
                tree.collapse(&dir);
                Ok(())
            });
        } else if let Some(parent_ix) = entry
            .path
            .ancestors()
            .skip(1)
            .find_map(|ancestor| self.row_ix(ancestor).filter(|parent| *parent != ix))
        {
            self.select(parent_ix, cx);
        }
    }

    fn open_selected(&mut self, _: &OpenSelected, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_none()
            && let Some(ix) = self.selected_ix()
        {
            self.click_row(ix, Modifiers::default(), window, cx);
        }
    }

    fn collapse_all(&mut self, _: &CollapseAll, window: &mut Window, cx: &mut Context<Self>) {
        self.update_tree(window, cx, |tree| {
            tree.collapse_all();
            Ok(())
        });
    }

    /// Escape: closes, in order, the name box, the filter, a pending cut, a
    /// multi-selection; otherwise passes on.
    fn cancel(&mut self, _: &CancelEdit, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            self.end_edit(window, cx);
        } else if self.filter.is_some() {
            self.close_filter(window, cx);
        } else if self
            .clipboard
            .as_ref()
            .is_some_and(|clipboard| clipboard.cut)
        {
            self.clipboard = None;
            cx.notify();
        } else if self.marked.len() > 1 {
            self.marked = self.selected.iter().cloned().collect();
            cx.notify();
        } else {
            cx.propagate();
        }
    }

    fn is_expanded(&self, dir: &Path, cx: &App) -> bool {
        self.core
            .read(cx)
            .tree()
            .is_some_and(|tree| tree.is_expanded(dir))
    }

    /// Where the new-entry name box goes: (list index, depth). It is the first
    /// row inside its folder.
    fn new_slot(&self, cx: &App) -> Option<(usize, usize)> {
        let EditKind::New { dir, .. } = &self.edit.as_ref()?.kind else {
            return None;
        };
        if self.core.read(cx).tree()?.root() == dir.as_path() {
            return Some((0, 0));
        }
        let ix = self.row_ix(dir)?;
        Some((ix + 1, self.rows[ix].depth + 1))
    }

    fn render_tree(&self, name: SharedString, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .flex_none()
            .h(fixed::TREE_ROW_HEIGHT)
            .px(fixed::TREE_PADDING)
            .justify_between()
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            // Buttons call methods directly: a dispatched action would start
            // at the focused element, which may be outside the Explorer.
            .child(
                h_flex()
                    .flex_none()
                    .child(header_button(
                        "explorer-new-file",
                        IconName::FilePlus,
                        "New File...",
                        cx.listener(|this, _, window, cx| this.start_new(false, window, cx)),
                    ))
                    .child(header_button(
                        "explorer-new-folder",
                        IconName::FolderPlus,
                        "New Folder...",
                        cx.listener(|this, _, window, cx| this.start_new(true, window, cx)),
                    ))
                    .child(header_button(
                        "explorer-collapse-all",
                        IconName::ChevronsDownUp,
                        "Collapse Folders",
                        cx.listener(|this, _, window, cx| {
                            this.collapse_all(&CollapseAll, window, cx)
                        }),
                    )),
            );

        let filter_bar = self.filter.as_ref().map(|filter| {
            h_flex()
                .flex_none()
                .h(fixed::TREE_ROW_HEIGHT)
                .px(fixed::TREE_PADDING)
                .child(Input::new(&filter.input).xsmall().flex_1())
        });

        let slot = self.new_slot(cx);
        let count = self.rows.len() + usize::from(slot.is_some());
        let list = uniform_list(
            "explorer-rows",
            count,
            cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                range
                    .map(|list_ix| match slot {
                        Some((at, depth)) if list_ix == at => this.render_new_row(depth, cx),
                        Some((at, _)) if list_ix > at => this.render_row(list_ix - 1, window, cx),
                        _ => this.render_row(list_ix, window, cx),
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full();

        let explorer = cx.entity().downgrade();
        v_flex()
            .size_full()
            .child(header)
            .children(filter_bar)
            .child(
                div()
                    .id("explorer-list")
                    .flex_1()
                    .min_h_0()
                    .child(list)
                    .vertical_scrollbar(&self.scroll)
                    // Dropping on empty space moves (or copies) into the root. No
                    // highlight here: it would light up the whole list under a row.
                    .on_drop(
                        cx.listener(|this, drag: &rows::DraggedEntries, window, cx| {
                            this.drop_on(None, drag.paths.clone(), false, window, cx)
                        }),
                    )
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        this.drop_on(None, paths.paths().to_vec(), true, window, cx)
                    }))
                    // Right-click on empty space: the menu acts on the workspace root.
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, window, cx| {
                            if !std::mem::take(&mut this.row_right_clicked) {
                                this.selected = None;
                                this.marked.clear();
                                cx.notify();
                            }
                            window.focus(&this.focus_handle, cx);
                        }),
                    )
                    .context_menu(move |menu, _, cx| {
                        let state = explorer.upgrade().map(|explorer| {
                            let explorer = explorer.read(cx);
                            MenuState {
                                selected: explorer.selection().len(),
                                can_paste: explorer.clipboard.is_some(),
                                can_undo: explorer.core.read(cx).can_undo_file_op(),
                            }
                        });
                        context_menu(menu, state.unwrap_or_default())
                    }),
            )
    }
}

fn header_button(
    id: &'static str,
    icon: IconName,
    tooltip: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .tooltip(tooltip)
        .on_click(on_click)
}

/// Lets saved layouts recreate the Explorer; binds its keys.
pub fn register(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, Some(KEY_CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-up", ExtendPrev, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-down", ExtendNext, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-a", SelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("right", ExpandOrNext, Some(KEY_CONTEXT)),
        KeyBinding::new("left", CollapseOrParent, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", OpenSelected, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", CancelEdit, Some(KEY_CONTEXT)),
        KeyBinding::new("f2", Rename, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", Delete, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-c", CopyFiles, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-x", CutFiles, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-v", PasteFiles, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-z", UndoFileOp, Some(KEY_CONTEXT)),
        // VS Code's keys for these.
        KeyBinding::new("shift-alt-c", CopyPath, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-shift-alt-c", CopyRelativePath, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-alt-r", RevealInOs, Some(KEY_CONTEXT)),
    ]);
    register_panel(cx, PANEL_NAME, |_, _, cx| {
        panel_handle(ExplorerPanel::new(cx))
    });
}

/// What the right-click menu can offer.
#[derive(Clone, Copy, Default)]
struct MenuState {
    /// Selected entries (0 on empty space: the menu acts on the root).
    selected: usize,
    can_paste: bool,
    can_undo: bool,
}

fn context_menu(menu: PopupMenu, state: MenuState) -> PopupMenu {
    let none = state.selected == 0;
    let many = state.selected > 1;
    let delete = if many {
        format!("Delete {} Items", state.selected)
    } else {
        "Delete".to_string()
    };
    menu.menu("New File...", Box::new(NewFile))
        .menu("New Folder...", Box::new(NewFolder))
        .separator()
        .menu_with_disabled("Cut", Box::new(CutFiles), none)
        .menu_with_disabled("Copy", Box::new(CopyFiles), none)
        .menu_with_disabled("Paste", Box::new(PasteFiles), !state.can_paste)
        .separator()
        .menu_with_disabled("Rename", Box::new(Rename), none || many)
        .menu_with_disabled(delete, Box::new(Delete), none)
        .menu_with_disabled("Undo", Box::new(UndoFileOp), !state.can_undo)
        .separator()
        .menu("Copy Path", Box::new(CopyPath))
        .menu("Copy Relative Path", Box::new(CopyRelativePath))
        .separator()
        .menu(REVEAL_LABEL, Box::new(RevealInOs))
}

impl EventEmitter<PanelEvent> for ExplorerPanel {}

impl Focusable for ExplorerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ExplorerPanel {
    fn panel_name(&self) -> &'static str {
        PANEL_NAME
    }

    /// Never closes (user): it is the Explorer section's, which collapses
    /// but keeps its panels (`workbench.rs`).
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for ExplorerPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("Explorer".into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Explorer"
    }

    /// A tab even when alone (user, 2026-10-22): the plain title it had
    /// alone looked unlike the tab bar of other panels (no bar background,
    /// no tab borders). No x either way (`closable`).
    fn always_tab_bar(&self, _: &App) -> bool {
        true
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        crate::workbench::close_panel(cx.entity(), window, cx);
        true
    }

    /// The section's side (user, 2026-10-23): checked when it is on the right.
    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        menu.menu_with_check(
            "Section on Right",
            crate::workbench::section_on_right(cx),
            Box::new(ToggleSectionSide),
        )
    }
}

impl Render for ExplorerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let body =
            v_flex()
                .track_focus(&self.focus_handle)
                .key_context(KEY_CONTEXT)
                .on_key_down(cx.listener(Self::type_to_filter))
                .on_action(cx.listener(Self::select_prev))
                .on_action(cx.listener(Self::select_next))
                .on_action(cx.listener(Self::extend_prev))
                .on_action(cx.listener(Self::extend_next))
                .on_action(cx.listener(Self::select_all))
                .on_action(cx.listener(Self::expand_or_next))
                .on_action(cx.listener(Self::collapse_or_parent))
                .on_action(cx.listener(Self::open_selected))
                .on_action(cx.listener(Self::collapse_all))
                .on_action(cx.listener(Self::cancel))
                .on_action(cx.listener(Self::new_file))
                .on_action(cx.listener(Self::new_folder))
                .on_action(cx.listener(Self::rename))
                .on_action(cx.listener(Self::delete))
                .on_action(cx.listener(Self::copy_files))
                .on_action(cx.listener(Self::cut_files))
                .on_action(cx.listener(Self::paste_files))
                .on_action(cx.listener(Self::undo))
                // Edit menu items act on files here (text inputs inside handle
                // them first).
                .on_action(cx.listener(|this, _: &input::Cut, window, cx| {
                    this.cut_files(&CutFiles, window, cx)
                }))
                .on_action(cx.listener(|this, _: &input::Copy, window, cx| {
                    this.copy_files(&CopyFiles, window, cx)
                }))
                .on_action(cx.listener(|this, _: &input::Paste, window, cx| {
                    this.paste_files(&PasteFiles, window, cx)
                }))
                .on_action(cx.listener(|this, _: &input::Undo, window, cx| {
                    this.undo(&UndoFileOp, window, cx)
                }))
                .on_action(cx.listener(Self::copy_path))
                .on_action(cx.listener(Self::copy_relative_path))
                .on_action(cx.listener(Self::reveal))
                .size_full();

        let Some(workspace) = self.core.read(cx).workspace() else {
            return body
                .p(fixed::PANEL_PADDING)
                .gap(fixed::PANEL_GAP)
                .items_center()
                .justify_center()
                .text_center()
                .child("No folder open")
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("Open a folder or create a new one to start."),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .justify_center()
                        .child(
                            Button::new("explorer-open-folder")
                                .primary()
                                .label("Open Folder")
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(OpenFolder), cx)
                                }),
                        )
                        .child(
                            Button::new("explorer-create-folder")
                                .label("Create Folder")
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(CreateFolder), cx)
                                }),
                        ),
                )
                .into_any_element();
        };

        let name: SharedString = workspace.name().to_uppercase().into();
        body.child(self.render_tree(name, cx)).into_any_element()
    }
}
