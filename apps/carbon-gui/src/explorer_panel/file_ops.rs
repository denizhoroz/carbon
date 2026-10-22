//! Explorer file operations: new file / folder and rename (inline name box),
//! delete to the Recycle Bin / Trash (confirmed), copy / cut / paste, drag
//! and drop, undo, copy path, reveal in the OS file manager.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use carbon_core::{CoreError, EntryKind};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, WindowExt, h_flex, v_flex};
use gpui_kit::*;

use super::{
    CopyFiles, CopyPath, CopyRelativePath, CutFiles, Delete, ExplorerPanel, NewFile, NewFolder,
    PasteFiles, Rename, RevealInOs, UndoFileOp,
};
use crate::style::fixed;
use crate::workbench::WindowCallback;

/// Label of the reveal command, in the platform's words.
pub const REVEAL_LABEL: &str = if cfg!(windows) {
    "Reveal in File Explorer"
} else if cfg!(target_os = "macos") {
    "Reveal in Finder"
} else {
    "Open Containing Folder"
};

pub(super) const TRASH_LABEL: &str = if cfg!(windows) {
    "Recycle Bin"
} else {
    "Trash"
};

/// What the inline name box is doing.
#[derive(Clone)]
pub enum EditKind {
    /// A new entry in `dir` (shown as an extra row at the top of it).
    New {
        dir: PathBuf,
        folder: bool,
    },
    Rename {
        path: PathBuf,
    },
}

pub struct Edit {
    pub kind: EditKind,
    pub input: Entity<InputState>,
    _events: Subscription,
}

/// Explorer copy / cut, kept inside the app (not the OS clipboard).
pub struct FileClipboard {
    pub paths: Vec<PathBuf>,
    /// Cut: Paste moves (and the rows show faded until then).
    pub cut: bool,
}

/// "'name'" for one path, "N items" for more.
fn describe(paths: &[PathBuf]) -> String {
    match paths {
        [one] => format!(
            "'{}'",
            one.file_name().unwrap_or_default().to_string_lossy()
        ),
        many => format!("{} items", many.len()),
    }
}

impl ExplorerPanel {
    /// The folder new entries and pastes go in: the cursor folder, the cursor
    /// file's folder, or the workspace root.
    fn target_dir(&self, cx: &App) -> Option<PathBuf> {
        let root = self.core.read(cx).tree()?.root().to_path_buf();
        let Some(row) = self.selected_ix().map(|ix| &self.rows[ix]) else {
            return Some(root);
        };
        Some(match row.entry.kind {
            EntryKind::Dir => row.entry.path.clone(),
            _ => row.entry.path.parent().map_or(root, Path::to_path_buf),
        })
    }

    pub(super) fn new_file(&mut self, _: &NewFile, window: &mut Window, cx: &mut Context<Self>) {
        self.start_new(false, window, cx);
    }

    pub(super) fn new_folder(
        &mut self,
        _: &NewFolder,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_new(true, window, cx);
    }

    pub(super) fn start_new(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dir) = self.target_dir(cx) else {
            return;
        };
        let root = self
            .core
            .read(cx)
            .tree()
            .map(|tree| tree.root().to_path_buf());
        if root.as_deref() != Some(dir.as_path()) {
            let open = dir.clone();
            self.update_tree(window, cx, |tree| tree.expand(&open));
        }
        self.start_edit(EditKind::New { dir, folder }, "", window, cx);
    }

    pub(super) fn rename(&mut self, _: &Rename, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.selected_ix() else {
            return;
        };
        let entry = &self.rows[ix].entry;
        let (path, name) = (entry.path.clone(), entry.name.clone());
        let is_file = entry.kind != EntryKind::Dir;
        self.start_edit(EditKind::Rename { path }, &name, window, cx);
        // Select the name without its extension, like VS Code.
        let stem = match name.rfind('.') {
            Some(dot) if is_file && dot > 0 => dot,
            _ => name.len(),
        };
        if let Some(edit) = &self.edit {
            edit.input
                .update(cx, |input, cx| input.set_selected_range(0..stem, cx));
        }
    }

    fn start_edit(
        &mut self,
        kind: EditKind,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.to_string();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let _events = cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            // Enter keeps the box open on an error so the name can be fixed;
            // clicking away gives up on a bad name.
            InputEvent::PressEnter { .. } => this.commit_edit(true, window, cx),
            InputEvent::Blur => this.commit_edit(false, window, cx),
            _ => {}
        });
        self.edit = Some(Edit {
            kind,
            input: input.clone(),
            _events,
        });
        cx.notify();
        // Deferred: the click that started this may still move focus.
        window.defer(cx, move |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx))
        });
    }

    fn commit_edit(&mut self, keep_on_error: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = &self.edit else {
            return;
        };
        let name = edit.input.read(cx).value().trim().to_string();
        let kind = edit.kind.clone();
        let unchanged = match &kind {
            EditKind::Rename { path } => path.file_name().is_some_and(|old| *old == *name),
            EditKind::New { .. } => false,
        };
        if name.is_empty() || unchanged {
            return self.end_edit(window, cx);
        }
        let result = self.core.update(cx, |core, cx| {
            let result = match &kind {
                EditKind::New { dir, folder: false } => core.create_file(dir, &name),
                EditKind::New { dir, folder: true } => core.create_dir(dir, &name),
                EditKind::Rename { path } => core.rename(path, &name),
            };
            cx.notify();
            result
        });
        match result {
            Ok(path) => {
                self.end_edit(window, cx);
                self.select_paths(vec![path.clone()], cx);
                if matches!(kind, EditKind::New { folder: false, .. }) {
                    self.open_file(&path, window, cx);
                }
            }
            Err(err) => {
                let what = match kind {
                    EditKind::New { folder: false, .. } => "create file",
                    EditKind::New { folder: true, .. } => "create folder",
                    EditKind::Rename { .. } => "rename",
                };
                // The name is enough here; the full path is long and known.
                let reason = match err {
                    CoreError::AlreadyExists(_) => format!("'{name}' already exists"),
                    err => err.to_string(),
                };
                window.push_notification(format!("Cannot {what}: {reason}"), cx);
                if !keep_on_error {
                    self.end_edit(window, cx);
                }
            }
        }
    }

    /// Closes the name box (its subscription goes with it) and gives focus back
    /// to the tree.
    pub(super) fn end_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Asks first (unless the Confirm Delete setting is off), then moves the
    /// selected entries to the Recycle Bin / Trash.
    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            return;
        }
        let paths = self.selection();
        let Some(first) = paths.first() else {
            return;
        };
        let question = match paths.as_slice() {
            [_] if first.is_dir() => {
                format!("Delete folder {} and its contents?", describe(&paths))
            }
            _ => format!("Delete {}?", describe(&paths)),
        };
        let core = self.core.clone();
        // Enter (the dialog's Confirm) and the button both run this.
        let confirm: WindowCallback = Rc::new(move |window, cx| {
            let result = core.update(cx, |core, cx| {
                let result = paths.iter().try_for_each(|path| core.trash(path));
                cx.notify();
                result
            });
            if let Err(err) = result {
                window.push_notification(format!("Cannot delete: {err}"), cx);
            }
        });
        if !crate::settings::get(cx).get_bool(super::CONFIRM_DELETE) {
            confirm(window, cx);
            return;
        }
        window.open_dialog(cx, move |dialog, _, cx| {
            let on_ok = confirm.clone();
            let on_click = confirm.clone();
            dialog
                .title("Delete")
                .w(fixed::DIALOG_WIDTH)
                .on_ok(move |_, window, cx| {
                    on_ok(window, cx);
                    true
                })
                .child(
                    v_flex()
                        .gap(fixed::PANEL_GAP)
                        .child(question.clone())
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("You can restore from the {TRASH_LABEL}.")),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("delete-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("delete-ok")
                                .danger()
                                .label(format!("Move to {TRASH_LABEL}"))
                                .on_click(move |_, window, cx| {
                                    on_click(window, cx);
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    pub(super) fn copy_files(&mut self, _: &CopyFiles, _: &mut Window, cx: &mut Context<Self>) {
        self.set_clipboard(false, cx);
    }

    pub(super) fn cut_files(&mut self, _: &CutFiles, _: &mut Window, cx: &mut Context<Self>) {
        self.set_clipboard(true, cx);
    }

    fn set_clipboard(&mut self, cut: bool, cx: &mut Context<Self>) {
        let paths = self.selection();
        if self.edit.is_none() && !paths.is_empty() {
            self.clipboard = Some(FileClipboard { paths, cut });
            cx.notify();
        }
    }

    /// Paste into the target folder: copies stay on the clipboard (paste again
    /// for another copy), a cut is used up.
    pub(super) fn paste_files(
        &mut self,
        _: &PasteFiles,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.is_some() {
            return;
        }
        let (Some(clipboard), Some(dir)) = (&self.clipboard, self.target_dir(cx)) else {
            return;
        };
        let (paths, cut) = (clipboard.paths.clone(), clipboard.cut);
        if cut {
            self.clipboard = None;
        }
        self.transfer(dir, paths, !cut, window, cx);
    }

    /// Drop of Explorer rows or OS files on a folder (`None`: the root). Rows
    /// move, or copy with Ctrl held (Option on macOS); OS files always copy.
    pub(super) fn drop_on(
        &mut self,
        dir: Option<PathBuf>,
        paths: Vec<PathBuf>,
        external: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dir) = dir.or_else(|| Some(self.core.read(cx).tree()?.root().to_path_buf()))
        else {
            return;
        };
        let modifiers = window.modifiers();
        let copy_key = if cfg!(target_os = "macos") {
            modifiers.alt
        } else {
            modifiers.control
        };
        self.transfer(dir, paths, external || copy_key, window, cx);
    }

    fn transfer(
        &mut self,
        dir: PathBuf,
        paths: Vec<PathBuf>,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = self.core.update(cx, |core, cx| {
            let result = if copy {
                core.copy_entries(&paths, &dir)
            } else {
                core.move_entries(&paths, &dir)
            };
            cx.notify();
            result
        });
        match result {
            Ok(new_paths) => self.select_paths(new_paths, cx),
            Err(err) => {
                let what = if copy { "copy" } else { "move" };
                let reason = match err {
                    CoreError::AlreadyExists(path) => format!(
                        "'{}' already exists there",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    err => err.to_string(),
                };
                window
                    .push_notification(format!("Cannot {what} {}: {reason}", describe(&paths)), cx);
            }
        }
    }

    /// Ctrl+Z: reverses the newest create / rename / move / copy.
    pub(super) fn undo(&mut self, _: &UndoFileOp, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            return;
        }
        let result = self.core.update(cx, |core, cx| {
            let result = core.undo_file_op();
            cx.notify();
            result
        });
        match result {
            Ok(op) => window.push_notification(format!("Undid {}", op.label()), cx),
            Err(CoreError::NothingToUndo) => {}
            Err(err) => window.push_notification(err.to_string(), cx),
        }
    }

    /// Selected entries, or the workspace root when nothing is selected.
    fn selected_or_root(&self, cx: &App) -> Vec<PathBuf> {
        let selection = self.selection();
        if selection.is_empty() {
            self.core
                .read(cx)
                .tree()
                .map(|tree| vec![tree.root().to_path_buf()])
                .unwrap_or_default()
        } else {
            selection
        }
    }

    /// One path per line (several when several rows are selected).
    pub(super) fn copy_path(&mut self, _: &CopyPath, _: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_or_root(cx);
        if !paths.is_empty() {
            let text: Vec<String> = paths
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            cx.write_to_clipboard(ClipboardItem::new_string(text.join("\n")));
        }
    }

    pub(super) fn copy_relative_path(
        &mut self,
        _: &CopyRelativePath,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.selected_or_root(cx);
        let core = self.core.read(cx);
        let Some(workspace) = core.workspace() else {
            return;
        };
        let text: Vec<String> = paths
            .iter()
            .map(|path| {
                let relative = workspace
                    .relative(path)
                    .map(|relative| relative.display().to_string())
                    .unwrap_or_default();
                // The root itself is "." (an empty string would look like nothing was copied).
                if relative.is_empty() {
                    ".".into()
                } else {
                    relative
                }
            })
            .collect();
        cx.write_to_clipboard(ClipboardItem::new_string(text.join("\n")));
    }

    pub(super) fn reveal(&mut self, _: &RevealInOs, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self
            .selected
            .clone()
            .or_else(|| self.selected_or_root(cx).pop())
        else {
            return;
        };
        if let Err(err) = reveal_in_os(&path) {
            window.push_notification(format!("Cannot reveal {}: {err}", path.display()), cx);
        }
    }
}

/// Opens the OS file manager with `path` selected (Linux: its folder).
fn reveal_in_os(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // `explorer` wants `/select,"path"` as one raw argument.
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path.parent().unwrap_or(path))
            .spawn()?;
    }
    Ok(())
}
