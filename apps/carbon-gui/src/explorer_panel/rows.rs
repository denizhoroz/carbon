//! Drawing one Explorer row: indent, chevron, icon, name, markers (problems,
//! agent edits, git state), selection, and drag and drop.

use std::path::{Path, PathBuf};

use carbon_core::{EntryKind, GitStatus, Mark};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, Theme, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ExplorerPanel;
use super::file_ops::EditKind;
use crate::style::fixed;

/// Explorer entries being dragged (the selection, or the row grabbed).
#[derive(Clone)]
pub struct DraggedEntries {
    pub paths: Vec<PathBuf>,
    label: SharedString,
}

/// The chip that follows the pointer while dragging.
impl Render for DraggedEntries {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .px(fixed::TREE_PADDING)
            .py(fixed::TREE_ITEM_GAP)
            .rounded_md()
            .border_1()
            .border_color(theme.drag_border)
            .bg(theme.popover)
            .text_sm()
            .child(self.label.clone())
    }
}

fn git_color(status: GitStatus, theme: &Theme) -> Hsla {
    match status {
        GitStatus::Untracked | GitStatus::Added => theme.success,
        GitStatus::Modified | GitStatus::Renamed => theme.warning,
        GitStatus::Deleted | GitStatus::Conflict => theme.danger,
    }
}

impl ExplorerPanel {
    pub(super) fn render_row(
        &self,
        ix: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let row = &self.rows[ix];
        let entry = &row.entry;
        let is_dir = entry.kind == EntryKind::Dir;
        let core = self.core.read(cx);
        let (git, mark) = if is_dir {
            (
                core.git().and_then(|git| git.dir(&entry.path)),
                core.marks.dir(&entry.path),
            )
        } else {
            (
                core.git().and_then(|git| git.file(&entry.path)),
                core.marks.file(&entry.path),
            )
        };
        let theme = cx.theme();
        let expanded = is_dir && self.is_expanded(&entry.path, cx);
        let marked = self.is_marked(&entry.path);
        let cursor = self.selected.as_ref() == Some(&entry.path);
        let focused = self.focus_handle.contains_focused(window, cx);
        let cut = self
            .clipboard
            .as_ref()
            .is_some_and(|clipboard| clipboard.cut && clipboard.paths.contains(&entry.path));
        let (icon, chevron) = match entry.kind {
            EntryKind::Dir if expanded => (IconName::FolderOpen, Some(IconName::ChevronDown)),
            EntryKind::Dir => (IconName::Folder, Some(IconName::ChevronRight)),
            EntryKind::File => (IconName::File, None),
            EntryKind::Symlink => (IconName::FileSymlink, None),
        };
        // Errors win, then git state, then problems' warnings, then ignored.
        let text = if mark.errors > 0 {
            theme.danger
        } else if let Some(git) = git {
            git_color(git, theme)
        } else if mark.warnings > 0 {
            theme.warning
        } else if entry.ignored {
            theme.muted_foreground
        } else {
            theme.foreground
        };
        let icon_size = Size::Size(fixed::TREE_ICON_SIZE);
        let drop_color = theme.drop_target;
        let drop_dir = if is_dir {
            Some(entry.path.clone())
        } else {
            entry.path.parent().map(PathBuf::from)
        };
        let drop_dir_external = drop_dir.clone();
        let dragged = self.dragged_for(&entry.path, &row.label);

        h_flex()
            .id(ix)
            .h(fixed::TREE_ROW_HEIGHT)
            .w_full()
            .pl(fixed::TREE_PADDING + fixed::TREE_INDENT * row.depth as f32)
            .pr(fixed::TREE_PADDING)
            .gap(fixed::TREE_ITEM_GAP)
            .text_sm()
            .text_color(text)
            .when(cut, |this| this.opacity(0.5))
            .when(marked, |this| this.bg(theme.list_active))
            .when(cursor && focused, |this| {
                this.border_1().border_color(theme.list_active_border)
            })
            .when(!marked, |this| this.hover(|style| style.bg(theme.list_hover)))
            .child(
                div()
                    .flex_none()
                    .size(fixed::TREE_ICON_SIZE)
                    .when_some(chevron, |this, chevron| {
                        this.child(
                            Icon::new(chevron)
                                .with_size(icon_size)
                                .text_color(theme.muted_foreground),
                        )
                    }),
            )
            .child(
                Icon::new(icon)
                    .with_size(icon_size)
                    .flex_none()
                    .text_color(theme.muted_foreground),
            )
            .map(|this| match &self.edit {
                Some(edit) if matches!(&edit.kind, EditKind::Rename { path } if *path == entry.path) => {
                    this.child(name_box(&edit.input))
                }
                _ => this.child(div().flex_1().min_w_0().truncate().child(row.label.clone())),
            })
            .children(markers(is_dir, mark, git, theme))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                this.click_row(ix, event.modifiers(), window, cx)
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, window, cx| {
                    this.row_right_clicked = true;
                    // Right-click inside a multi-selection keeps it (menu acts on all).
                    if !this.rows.get(ix).is_some_and(|row| this.is_marked(&row.entry.path)) {
                        this.select(ix, cx);
                    }
                    window.focus(&this.focus_handle, cx);
                }),
            )
            .on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .drag_over::<DraggedEntries>(move |style, _, _, _| style.bg(drop_color))
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(drop_color))
            .on_drop(cx.listener(move |this, drag: &DraggedEntries, window, cx| {
                this.drop_on(drop_dir.clone(), drag.paths.clone(), false, window, cx)
            }))
            .on_drop(cx.listener(move |this, paths: &ExternalPaths, window, cx| {
                this.drop_on(drop_dir_external.clone(), paths.paths().to_vec(), true, window, cx)
            }))
    }

    /// Dragging a selected row drags the whole selection.
    fn dragged_for(&self, path: &Path, label: &str) -> DraggedEntries {
        if self.is_marked(path) && self.marked.len() > 1 {
            DraggedEntries {
                paths: self.selection(),
                label: format!("{} items", self.marked.len()).into(),
            }
        } else {
            DraggedEntries {
                paths: vec![path.to_path_buf()],
                label: label.to_string().into(),
            }
        }
    }

    /// The name box row for a new entry, indented like the entries it joins.
    pub(super) fn render_new_row(&self, depth: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let folder = matches!(
            self.edit.as_ref().map(|edit| &edit.kind),
            Some(EditKind::New { folder: true, .. })
        );
        let icon = if folder {
            IconName::Folder
        } else {
            IconName::File
        };
        h_flex()
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
            )
            .when_some(self.edit.as_ref(), |this, edit| {
                this.child(name_box(&edit.input))
            })
    }
}

/// The inline name box for new and renamed entries: the row's full height,
/// the plain input border (no focus ring), no placeholder.
fn name_box(input: &Entity<InputState>) -> Input {
    Input::new(input).small().focus_bordered(false).flex_1()
}

/// Right side of a row: agent mark, problem counts, git letter. Folders show
/// dots for what is inside them instead of counts and letters.
fn markers(is_dir: bool, mark: Mark, git: Option<GitStatus>, theme: &Theme) -> Vec<AnyElement> {
    let mut out = Vec::new();
    if mark.agent_edited {
        out.push(
            Icon::new(IconName::Bot)
                .with_size(Size::Size(fixed::TREE_MARK_ICON_SIZE))
                .flex_none()
                .text_color(theme.info)
                .into_any_element(),
        );
    }
    let problems = if mark.errors > 0 {
        Some((mark.errors, theme.danger))
    } else if mark.warnings > 0 {
        Some((mark.warnings, theme.warning))
    } else {
        None
    };
    if let Some((count, color)) = problems {
        out.push(if is_dir {
            dot(color)
        } else {
            badge(count.to_string(), color)
        });
    }
    if let Some(git) = git {
        let color = git_color(git, theme);
        out.push(if is_dir {
            dot(color)
        } else {
            badge(git.letter().to_string(), color)
        });
    }
    out
}

fn badge(text: String, color: Hsla) -> AnyElement {
    div()
        .flex_none()
        .text_xs()
        .text_color(color)
        .child(text)
        .into_any_element()
}

fn dot(color: Hsla) -> AnyElement {
    div()
        .flex_none()
        .size(fixed::TREE_DOT_SIZE)
        .rounded_full()
        .bg(color)
        .into_any_element()
}
