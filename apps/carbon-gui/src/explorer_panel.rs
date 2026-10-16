//! Explorer panel. Empty state: Open Folder / Create Folder (DESIGN.md s4:
//! "no workspace = Open and Create buttons"). With a folder open: the file
//! tree, a flat virtualized row list over `carbon_core::FileTree` (folders load
//! when they expand; the core's watcher keeps loaded folders current).
//! File operations (new, rename, delete, copy path, reveal) are in `file_ops`.

mod file_ops;

use std::path::{Path, PathBuf};

use carbon_core::{Core, EntryKind, Row};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, panel_handle, register_panel};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu};
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size, WindowExt, h_flex, scroll::ScrollableElement, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::menus::{CreateFolder, OpenFolder};
use crate::style::fixed;
use crate::workspace_ops::current_core;
use file_ops::{Edit, EditKind, REVEAL_LABEL};

/// Persistence key in saved layouts. Never change it.
const PANEL_NAME: &str = "Explorer";
const KEY_CONTEXT: &str = "Explorer";

actions!(
    explorer,
    [
        SelectPrev,
        SelectNext,
        ExpandOrNext,
        CollapseOrParent,
        OpenSelected,
        CollapseAll,
        NewFile,
        NewFolder,
        Rename,
        Delete,
        CopyPath,
        CopyRelativePath,
        RevealInOs,
        CancelEdit
    ]
);

pub struct ExplorerPanel {
    core: Entity<Core>,
    focus_handle: FocusHandle,
    /// Visible rows, rebuilt when the core changes (not on every frame).
    rows: Vec<Row>,
    /// Kept by path, so it survives rows moving after a refresh.
    selected: Option<PathBuf>,
    scroll: UniformListScrollHandle,
    /// The inline name box (new entry or rename), if open.
    edit: Option<Edit>,
    /// Set by a row's right-click so the list's handler keeps that selection.
    row_right_clicked: bool,
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
                scroll: UniformListScrollHandle::new(),
                edit: None,
                row_right_clicked: false,
                _core_sub,
            };
            this.rebuild_rows(cx);
            this
        })
    }

    fn rebuild_rows(&mut self, cx: &App) {
        self.rows = self
            .core
            .read(cx)
            .tree()
            .map(|tree| tree.visible_rows())
            .unwrap_or_default();
        if let Some(selected) = &self.selected
            && !self.rows.iter().any(|row| &row.entry.path == selected)
        {
            self.selected = None;
        }
    }

    fn selected_ix(&self) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        self.rows.iter().position(|row| &row.entry.path == selected)
    }

    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get(ix) {
            self.selected = Some(row.entry.path.clone());
            let list_ix = match self.new_slot(cx) {
                Some((slot, _)) if ix >= slot => ix + 1,
                _ => ix,
            };
            self.scroll.scroll_to_item(list_ix, ScrollStrategy::Nearest);
            cx.notify();
        }
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
        let result = self.core.update(cx, |core, cx| {
            let result = core.open_file(path).map(|_| ());
            cx.notify();
            result
        });
        if let Err(err) = result {
            window.push_notification(format!("Cannot open file: {err}"), cx);
        }
    }

    /// Click: folders toggle, files open (no preview tabs yet).
    fn click_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(ix) else {
            return;
        };
        let (path, kind) = (row.entry.path.clone(), row.entry.kind);
        self.select(ix, cx);
        window.focus(&self.focus_handle, cx);
        match kind {
            EntryKind::Dir => self.toggle(path, window, cx),
            EntryKind::File | EntryKind::Symlink => self.open_file(&path, window, cx),
        }
    }

    fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            return;
        }
        let ix = self.selected_ix().map_or(0, |ix| ix.saturating_sub(1));
        self.select(ix, cx);
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            return;
        }
        let ix = self
            .selected_ix()
            .map_or(0, |ix| (ix + 1).min(self.rows.len().saturating_sub(1)));
        self.select(ix, cx);
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

    /// Left: close an open folder, else jump to the parent folder.
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
        } else if let Some(parent) = entry.path.parent()
            && let Some(parent_ix) = self.rows.iter().position(|row| row.entry.path == parent)
        {
            self.select(parent_ix, cx);
        }
    }

    fn open_selected(&mut self, _: &OpenSelected, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            return;
        }
        if let Some(ix) = self.selected_ix() {
            self.click_row(ix, window, cx);
        }
    }

    fn collapse_all(&mut self, _: &CollapseAll, window: &mut Window, cx: &mut Context<Self>) {
        self.update_tree(window, cx, |tree| {
            tree.collapse_all();
            Ok(())
        });
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
        let ix = self.rows.iter().position(|row| &row.entry.path == dir)?;
        Some((ix + 1, self.rows[ix].depth + 1))
    }

    fn is_expanded(&self, dir: &Path, cx: &App) -> bool {
        self.core
            .read(cx)
            .tree()
            .is_some_and(|tree| tree.is_expanded(dir))
    }

    fn render_row(&self, ix: usize, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let row = &self.rows[ix];
        let theme = cx.theme();
        let entry = &row.entry;
        let expanded = entry.kind == EntryKind::Dir && self.is_expanded(&entry.path, cx);
        let selected = self.selected.as_ref() == Some(&entry.path);
        let focused = self.focus_handle.contains_focused(window, cx);
        let (icon, chevron) = match entry.kind {
            EntryKind::Dir if expanded => (IconName::FolderOpen, Some(IconName::ChevronDown)),
            EntryKind::Dir => (IconName::Folder, Some(IconName::ChevronRight)),
            EntryKind::File => (IconName::File, None),
            EntryKind::Symlink => (IconName::FileSymlink, None),
        };
        let text = if entry.ignored {
            theme.muted_foreground
        } else {
            theme.foreground
        };
        let icon_size = Size::Size(fixed::TREE_ICON_SIZE);

        h_flex()
            .id(ix)
            .h(fixed::TREE_ROW_HEIGHT)
            .w_full()
            .pl(fixed::TREE_PADDING + fixed::TREE_INDENT * row.depth as f32)
            .pr(fixed::TREE_PADDING)
            .gap(fixed::TREE_ITEM_GAP)
            .text_sm()
            .text_color(text)
            .when(selected, |this| {
                this.bg(theme.list_active).when(focused, |this| {
                    this.border_1().border_color(theme.list_active_border)
                })
            })
            .when(!selected, |this| {
                this.hover(|style| style.bg(theme.list_hover))
            })
            .child(div().flex_none().size(fixed::TREE_ICON_SIZE).when_some(
                chevron,
                |this, chevron| {
                    this.child(
                        Icon::new(chevron)
                            .with_size(icon_size)
                            .text_color(theme.muted_foreground),
                    )
                },
            ))
            .child(
                Icon::new(icon)
                    .with_size(icon_size)
                    .flex_none()
                    .text_color(theme.muted_foreground),
            )
            .map(|this| match &self.edit {
                Some(edit) if matches!(&edit.kind, EditKind::Rename { path } if *path == entry.path) => {
                    this.child(Input::new(&edit.input).xsmall().flex_1())
                }
                _ => this.child(div().min_w_0().truncate().child(entry.name.clone())),
            })
            .on_click(cx.listener(move |this, _, window, cx| this.click_row(ix, window, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, window, cx| {
                    this.row_right_clicked = true;
                    this.select(ix, cx);
                    window.focus(&this.focus_handle, cx);
                }),
            )
    }

    /// The name box row for a new entry, indented like the entries it joins.
    fn render_new_row(&self, depth: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let folder = matches!(
            self.edit.as_ref().map(|edit| &edit.kind),
            Some(EditKind::New { folder: true, .. })
        );
        let icon = if folder {
            IconName::Folder
        } else {
            IconName::File
        };
        let mut row = h_flex()
            .id("explorer-new-entry")
            .h(fixed::TREE_ROW_HEIGHT)
            .w_full()
            .pl(fixed::TREE_PADDING + fixed::TREE_INDENT * depth as f32)
            .pr(fixed::TREE_PADDING)
            .gap(fixed::TREE_ITEM_GAP)
            .text_sm()
            .child(div().flex_none().size(fixed::TREE_ICON_SIZE))
            .child(
                Icon::new(icon)
                    .with_size(Size::Size(fixed::TREE_ICON_SIZE))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground),
            );
        if let Some(edit) = &self.edit {
            row = row.child(Input::new(&edit.input).xsmall().flex_1());
        }
        row
    }

    fn render_tree(&self, name: SharedString, cx: &mut Context<Self>) -> impl IntoElement {
        let header =
            h_flex()
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
                .child(
                    h_flex()
                        .flex_none()
                        .child(
                            Button::new("explorer-new-file")
                                .ghost()
                                .xsmall()
                                .icon(IconName::FilePlus)
                                .tooltip("New File...")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.start_new(false, window, cx)
                                })),
                        )
                        .child(
                            Button::new("explorer-new-folder")
                                .ghost()
                                .xsmall()
                                .icon(IconName::FolderPlus)
                                .tooltip("New Folder...")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.start_new(true, window, cx)
                                })),
                        )
                        // Called directly: a dispatched action would start at the
                        // focused element, which may be outside the Explorer.
                        .child(
                            Button::new("explorer-collapse-all")
                                .ghost()
                                .xsmall()
                                .icon(IconName::ChevronsDownUp)
                                .tooltip("Collapse Folders")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.collapse_all(&CollapseAll, window, cx)
                                })),
                        ),
                );

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
        v_flex().size_full().child(header).child(
            div()
                .id("explorer-list")
                .flex_1()
                .min_h_0()
                .child(list)
                .vertical_scrollbar(&self.scroll)
                // Right-click on empty space: the menu acts on the workspace root.
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, window, cx| {
                        if !std::mem::take(&mut this.row_right_clicked) {
                            this.selected = None;
                            cx.notify();
                        }
                        window.focus(&this.focus_handle, cx);
                    }),
                )
                .context_menu(move |menu, _, cx| {
                    let has_entry = explorer
                        .upgrade()
                        .is_some_and(|explorer| explorer.read(cx).selected.is_some());
                    context_menu(menu, has_entry)
                }),
        )
    }
}

/// Lets saved layouts recreate the Explorer; binds its keys.
pub fn register(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, Some(KEY_CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("right", ExpandOrNext, Some(KEY_CONTEXT)),
        KeyBinding::new("left", CollapseOrParent, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", OpenSelected, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", CancelEdit, Some(KEY_CONTEXT)),
        KeyBinding::new("f2", Rename, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", Delete, Some(KEY_CONTEXT)),
        // VS Code's keys for these.
        KeyBinding::new("shift-alt-c", CopyPath, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-shift-alt-c", CopyRelativePath, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-alt-r", RevealInOs, Some(KEY_CONTEXT)),
    ]);
    register_panel(cx, PANEL_NAME, |_, _, cx| {
        panel_handle(ExplorerPanel::new(cx))
    });
}

/// Right-click menu; `has_entry` is false on empty space (acts on the root).
fn context_menu(menu: PopupMenu, has_entry: bool) -> PopupMenu {
    menu.menu("New File...", Box::new(NewFile))
        .menu("New Folder...", Box::new(NewFolder))
        .separator()
        .menu_with_disabled("Rename", Box::new(Rename), !has_entry)
        .menu_with_disabled("Delete", Box::new(Delete), !has_entry)
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
}

impl Panel for ExplorerPanel {
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("Explorer".into())
    }

    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Explorer"
    }
}

impl Render for ExplorerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let body = v_flex()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::select_prev))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::expand_or_next))
            .on_action(cx.listener(Self::collapse_or_parent))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::collapse_all))
            .on_action(cx.listener(Self::new_file))
            .on_action(cx.listener(Self::new_folder))
            .on_action(cx.listener(Self::rename))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::copy_path))
            .on_action(cx.listener(Self::copy_relative_path))
            .on_action(cx.listener(Self::reveal))
            .on_action(cx.listener(Self::cancel_edit))
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
