# Explorer file tree (T1 view + live refresh, T2 file operations)

Built 2026-10-17. User request: plan the file tree, compare design alternatives, then build. User picked: own row list, folder-by-folder loading (background scan later), file watcher, T1 now / T2 next. T2 built the same day ("build T2").

## Design alternatives (compared before building)

| Question | Options | Picked | Why |
|---|---|---|---|
| Widget | A. gpui-kit `Tree` (nested `TreeItem`, kit flattens + virtualizes) / B. own flat row list over GPUI `uniform_list` (VS Code, Zed model) / A then B | **B** | Kit 0.7.3 `TreeItem::is_folder()` = "has children" (unloaded or empty folders look like files), `set_items` resets selection, single select, no inline rename, drag or row payload; T2/T3 would force a rewrite (read from `gpui-base-0.7.3/src/tree.rs`) |
| Data | Lazy per folder (VS Code) / full background scan (Zed worktree) / lazy now + scan later | **Lazy now, scan later** | `list_dir` existed; instant on huge repos; the scan comes with go-to-file and search |
| Freshness | Manual refresh / file watcher / poll | **Watcher** (`notify` 8.2, CC0, already in Cargo.lock via gpui) | Agents, git and other apps change files; must show live |
| Scope | T1 view / T2 file ops / T3 power | **T1 now, T2 next** | See tiers below |

| Tier | Features | State |
|---|---|---|
| T1 view | Expand/collapse, indent, chevrons, folder/file/symlink icons, ignored dimmed, selection, keyboard, open file, live refresh, collapse all | Built |
| T2 file ops | New file/folder (inline), rename (F2), delete to Recycle Bin, copy path / relative path, reveal in OS, context menu | Built |
| T3 power | Multi-select, drag-drop move, filter-on-type, compact single-child folders, file nesting, git / problems / agent-touched markers | Later |

## Behaviour

| Input | Result |
|---|---|
| Click folder | Select + expand/collapse |
| Click file | Select + open (core buffer; Editor placeholder shows "Opened: name", relative path, line count) |
| Up / Down | Move selection (list scrolls to it) |
| Right | Closed folder: expand; open folder: first child |
| Left | Open folder: collapse; else select parent folder |
| Enter | Same as click on the selected row |
| Header button (chevrons) | Collapse all |
| Header buttons | New File, New Folder, Collapse All |
| Right-click row / empty space | Menu: New File..., New Folder..., Rename (F2), Delete (Delete), Copy Path (Shift+Alt+C), Copy Relative Path (Ctrl+Shift+Alt+C), Reveal in File Explorer (Shift+Alt+R; "Reveal in Finder" / "Open Containing Folder" elsewhere). Empty space acts on the root; Rename/Delete greyed |
| New File / New Folder | Name box as the first row of the target folder (selected folder, selected file's folder, or root; folder opens). Enter creates (file: selected + opened); error keeps the box (notification: "'x' already exists"); click away creates, or drops a bad name; Escape cancels |
| F2 | Name box in the row, name selected without extension; Enter / click away renames; open buffers follow the new path (also inside a renamed folder) |
| Delete | Dialog "Delete 'x'?" / "Delete folder 'x' and its contents?", "You can restore it from the Recycle Bin"; Enter or "Move to Recycle Bin" moves it there (`trash` crate), Cancel/Escape keeps it. Open buffers stay; Editor shows "(deleted)" |
| Copy Path / Copy Relative Path / Reveal | Selected entry, or the root without a selection (relative path of the root = ".") |
| Disk change (any app) | Loaded folders re-list within ~100 ms; selection kept by path; deleted folders drop out with their subtree |

- Folders first, then case-insensitive names (core `list_dir`). `.git`, `.hg`, `.svn`, `CVS`, `.DS_Store`, `Thumbs.db` never shown.
- Ignored (`.gitignore`/`.ignore`) entries use `muted_foreground`; everything inside an ignored folder is ignored too.
- Collapsing a folder keeps its expanded subfolders; they reopen with it (VS Code behaviour). Collapse all forgets them.
- Header shows the workspace name in capitals (VS Code style).

## Code

| File | Role |
|---|---|
| `apps/carbon-core/src/file_tree.rs` | `FileTree` (root, expanded set, cached listings of loaded folders): `expand`, `collapse`, `toggle`, `collapse_all`, `refresh`, `refresh_all`, `visible_rows() -> Vec<Row{entry, depth}>`; ignored inherited from parent folder |
| `apps/carbon-core/src/file_ops.rs` | `validate_name`, `create_file`, `create_folder`, `rename` (same folder; case-only rename allowed), `move_to_trash` (`trash` 5.2.9, MIT); never overwrites |
| `apps/carbon-core/src/watcher.rs` | `FsWatcher`: `notify` recursive watch on the root; debounce thread (100 ms quiet) sends `FsChanges { dirs, rescan }` over `async_channel`; content edits (`Modify(Any/Data)`) and `.git` paths dropped |
| `apps/carbon-core/src/lib.rs` | `Core` owns tree + watcher (reset on open/close); `tree`, `tree_mut`, `fs_changes`, `apply_fs_changes`, `watch_error`, `active_buffer` (set by `open_file`); `create_file`, `create_dir`, `rename`, `trash` (inside the workspace only, never the root itself; tree refreshed at once); `BufferStore::moved` |
| `apps/carbon-gui/src/explorer_panel.rs` | Flat rows over `uniform_list` + kit scrollbar; key context `Explorer`; rows cached and rebuilt on core notify |
| `apps/carbon-gui/src/explorer_panel/file_ops.rs` | T2: inline name box (`Edit`, `EditKind`), delete dialog, clipboard, `reveal_in_os` (`explorer /select,"path"`, `open -R`, `xdg-open`) |
| `apps/carbon-gui/src/main.rs`, `workspace_ops.rs` | `CarbonApp::sync_watch` runs `apply_changes` task per watcher channel (restarts on a new workspace) |
| `apps/carbon-gui/src/editor_panel.rs` | Editor placeholder panel (`Editor`) showing the active buffer; old layouts' "Editor" TestPanel maps to it (no layout reset) |
| `apps/carbon-gui/src/style/fixed.rs` | `TREE_ROW_HEIGHT` 24, `TREE_INDENT` 12, `TREE_ICON_SIZE` 16, `TREE_PADDING` 8, `TREE_ITEM_GAP` 4 |
| `apps/carbon-gui/src/app_assets.rs` | Extra Lucide icons `File`, `FileSymlink`, `ChevronsDownUp`, `FilePlus`, `FolderPlus` |

## Verified

- Core tests `tests/explorer_tree.rs` (7): row order/depth, collapse keeps subtree, outside/file refused, refresh add/remove, rescan, ignored inheritance, close drops tree + active buffer, real watcher reports parent dir. Core tests `tests/file_ops.rs` (4 + 1 ignored): create file/folder show in tree, bad names / existing / outside refused, rename moves buffers (also in a renamed folder), case-only rename, root refused; the ignored test moves a real file to the Recycle Bin (run once with `--ignored`, passed). Workspace total 41 pass + 1 ignored; clippy, fmt clean.
- Click test 2026-10-17 (window messages, preview build): expand/collapse by click, open file → Editor shows name/path/lines, Up/Down/Left/Right/Enter, live add + delete on disk, collapse all, 3000-file folder opens instantly and keyboard scrolling follows selection, ignored folder children dimmed (fixed during test).
- T2 click test 2026-10-17: New File in `src` (header button, Enter, opened), F2 rename with stem selected (buffer followed), Delete dialog + Enter (Recycle Bin, Editor "(deleted)"), right-click row menu with shortcuts, right-click empty space (Rename/Delete greyed, New Folder in root), existing name keeps box + notification, Escape cancels, click-away commits, Copy Path / Copy Relative Path (clipboard checked), Reveal (File Explorer opened `src` with `lib.rs` selected). Fixed during test: missing header icons (not in `icon_assets!`), long "already exists" message, "1 lines".
- Not checked: keyboard shortcuts with modifiers (Shift+Alt+C/R, Ctrl+Shift+Alt+C; synthetic keys cannot hold modifiers), mouse-wheel scroll (synthetic wheel messages ignored by GPUI; list uses GPUI's standard scroll), double-click, macOS/Linux watchers.

## Limits

- Editing `.gitignore` re-dims only the folders it lists again (loaded subfolders keep old flags until they refresh).
- Symlinked folders are leaves (not expandable), so the tree cannot leave the workspace.
- Watch failure is silent in the UI (`Core::watch_error`; tree still works without live updates).
- No multi-select, drag-drop move, copy/paste of files, undo of file operations, filter or decorations yet (T3).
- Delete of a folder with open, unsaved buffers does not warn (buffers stay open, nothing is lost).
