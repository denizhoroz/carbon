# Explorer file tree (T1 view + live refresh, T2 file operations, T3 power features)

Built 2026-10-17. User request: plan the file tree, compare design alternatives, then build. User picked: own row list, folder-by-folder loading (background scan later), file watcher, T1 now / T2 next. T2 built the same day ("build T2"), then T3 ("build t3"). T3 user picks: git markers from gitoxide (`gix`), marker API for problems / agent edits now (no data sources yet), extras copy / cut / paste and undo; file nesting left out.

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
| T3 power | Multi-select, drag-drop move, filter-on-type, compact single-child folders, git / problems / agent-touched markers; copy / cut / paste, undo | Built (file nesting not built) |

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
| Ctrl+click / Shift+click (Cmd on macOS) | Add or remove one row / select a range; no open. Shift+Up/Down extend, Ctrl+A selects all visible rows. Delete, Copy, Cut, Copy Path and drag act on the whole selection ("Delete 3 items?"); Rename on the cursor row |
| Drag rows | Chip shows the name or "N items"; target row highlights. Drop on a folder row: into it; on a file row: into its folder; on empty space: into the root. Moves; Ctrl held (Option on macOS) copies. Into itself or onto a taken name is refused with a notification |
| Drop files from the OS (Windows Explorer, Finder) | Copied into the target folder (never moved) |
| Ctrl+C / Ctrl+X / Ctrl+V (menu: Cut, Copy, Paste) | Explorer-internal clipboard. Cut rows show faded until pasted (Escape cancels the cut). Paste goes into the cursor folder / cursor file's folder / root; copies get "name copy.ext", "name copy 2.ext"; a cut is used up, a copy can be pasted again |
| Ctrl+Z (menu: Undo) | Reverses the newest create, rename, move, copy or paste (history of 100). Created entries and copies go to the Recycle Bin; renames and moves go back. Notification "Undid move 'x'". Refused, with a reason, if the old name is taken again. Delete is not undone here (restore from the Recycle Bin) |
| Type a letter in the tree | Filter box opens under the header with that letter; rows whose name contains the text stay, with their folders. Up/Down/Enter work from the box; Escape (or emptying the box and clicking away) closes it. Only opened folders are searched |
| Compact folders | A folder whose only entry is a folder shares its row (`deep/a/b`); opening it opens the whole chain. The row stands for the deepest folder (toggle, rename, paste target). Core `FileTree::set_compact` (on by default; no setting in the UI yet) |
| Markers (right of the name) | Git: letter U untracked, A added, M modified, D deleted, R renamed, ! conflict, colored (green / yellow / red); name takes the color. Problems: error count (red) or warning count (yellow). Agent edited: bot icon. Folders show colored dots for anything inside instead |
| Escape order | Name box, filter, pending cut, multi-selection, then passes on |

- Folders first, then case-insensitive names (core `list_dir`). `.git`, `.hg`, `.svn`, `CVS`, `.DS_Store`, `Thumbs.db` never shown.
- Ignored (`.gitignore`/`.ignore`) entries use `muted_foreground`; everything inside an ignored folder is ignored too.
- Collapsing a folder keeps its expanded subfolders; they reopen with it (VS Code behaviour). Collapse all forgets them.
- Header shows the workspace name in capitals (VS Code style).

## Code

| File | Role |
|---|---|
| `apps/carbon-core/src/file_tree.rs` | `FileTree` (root, expanded set, cached listings of loaded folders): `expand`, `collapse`, `toggle`, `collapse_all`, `refresh`, `refresh_all`, `visible_rows() -> Vec<Row{entry, depth}>`; ignored inherited from parent folder |
| `apps/carbon-core/src/file_ops.rs` | `validate_name`, `create_file`, `create_folder`, `rename` (same folder; case-only rename allowed), `move_to_trash` (`trash` 5.2.9, MIT), `move_into` (all checks before any move), `copy_into` (free "copy" names; folder links not followed); never overwrites |
| `apps/carbon-core/src/git_status.rs` | `git_status(root)` with gitoxide (`gix` 0.89, features `status` + `sha1` + `parallel`; no network/merge/blame): index-worktree and tree-index changes, untracked files listed one by one; `GitStatusMap` (file state + strongest state per folder); `Ok(None)` outside a repository |
| `apps/carbon-core/src/marks.rs` | `Marks` on `Core::marks`: `set_problems`, `set_agent_edited`, folder roll-up, follows moves; filled by diagnostics / agents later |
| `apps/carbon-core/src/history.rs` | `FileOp` (Created, Renamed, Moved, Copied) with `label`; `HISTORY_LIMIT` 100 |
| `apps/carbon-core/src/watcher.rs` | `FsWatcher`: `notify` recursive watch on the root; debounce thread (100 ms quiet) sends `FsChanges { dirs, rescan }` over `async_channel`; content edits (`Modify(Any/Data)`) and `.git` paths dropped |
| `apps/carbon-core/src/lib.rs` | `Core` owns tree + watcher (reset on open/close); `tree`, `tree_mut`, `fs_changes`, `apply_fs_changes`, `watch_error`, `active_buffer` (set by `open_file`); `create_file`, `create_dir`, `rename`, `trash`, `move_entries`, `copy_entries` (sources may be outside: OS drops), `undo_file_op`, `can_undo_file_op` (inside the workspace only, never the root itself; tree refreshed at once); `git`/`set_git`; `marks`; `BufferStore::moved` |
| `apps/carbon-gui/src/explorer_panel.rs` | Flat rows over `uniform_list` + kit scrollbar; key context `Explorer`; rows cached and rebuilt on core notify |
| `apps/carbon-gui/src/explorer_panel.rs` | State (rows, cursor, marked, anchor, clipboard, filter), selection, keyboard, header, list, right-click menu, key bindings |
| `apps/carbon-gui/src/explorer_panel/rows.rs` | Row drawing, markers, `DraggedEntries` drag chip, drop targets |
| `apps/carbon-gui/src/explorer_panel/filter.rs` | Type to filter (`on_key_down`), `filter_rows` |
| `apps/carbon-gui/src/explorer_panel/file_ops.rs` | Inline name box (`Edit`, `EditKind`), delete dialog, copy / cut / paste (`FileClipboard`), drop, undo, path copy, `reveal_in_os` (`explorer /select,"path"`, `open -R`, `xdg-open`) |
| `apps/carbon-gui/src/main.rs`, `workspace_ops.rs` | `CarbonApp::sync_watch` runs `apply_changes` task per watcher channel (restarts on a new workspace): applies batches, runs `git_status` in the background at start and after `FsChanges::git` batches, at most once per second (`GIT_STATUS_INTERVAL`). Debug-build test hook `CARBON_TEST_MARKS="src/main.rs:2:1:agent;docs:0:3"` seeds markers |
| `apps/carbon-gui/src/editor_panel.rs` | Editor placeholder panel (`Editor`) showing the active buffer; old layouts' "Editor" TestPanel maps to it (no layout reset) |
| `apps/carbon-gui/src/style/fixed.rs` | `TREE_ROW_HEIGHT` 24, `TREE_INDENT` 12, `TREE_ICON_SIZE` 16, `TREE_PADDING` 8, `TREE_ITEM_GAP` 4 |
| `apps/carbon-gui/src/app_assets.rs` | Extra Lucide icons `File`, `FileSymlink`, `ChevronsDownUp`, `FilePlus`, `FolderPlus`, `Bot` |

## Verified

- Core tests `tests/explorer_tree.rs` (7): row order/depth, collapse keeps subtree, outside/file refused, refresh add/remove, rescan, ignored inheritance, close drops tree + active buffer, real watcher reports parent dir. Core tests `tests/file_ops.rs` (4 + 1 ignored): create file/folder show in tree, bad names / existing / outside refused, rename moves buffers (also in a renamed folder), case-only rename, root refused; the ignored test moves a real file to the Recycle Bin (run once with `--ignored`, passed). Workspace total 41 pass + 1 ignored; clippy, fmt clean.
- Click test 2026-10-17 (window messages, preview build): expand/collapse by click, open file → Editor shows name/path/lines, Up/Down/Left/Right/Enter, live add + delete on disk, collapse all, 3000-file folder opens instantly and keyboard scrolling follows selection, ignored folder children dimmed (fixed during test).
- T2 click test 2026-10-17: New File in `src` (header button, Enter, opened), F2 rename with stem selected (buffer followed), Delete dialog + Enter (Recycle Bin, Editor "(deleted)"), right-click row menu with shortcuts, right-click empty space (Rename/Delete greyed, New Folder in root), existing name keeps box + notification, Escape cancels, click-away commits, Copy Path / Copy Relative Path (clipboard checked), Reveal (File Explorer opened `src` with `lib.rs` selected). Fixed during test: missing header icons (not in `icon_assets!`), long "already exists" message, "1 lines".
- T3 core tests `tests/explorer_t3.rs` (7 + 1 ignored): compact chains, move (buffers follow; into itself / clash / outside refused, nothing moved), copy names (`copy`, `copy 2`, dotfile, folder, from outside), undo order and refusal, marks roll-up and moves, git status on a real repo made with the git CLI (modified, untracked, added, deleted, folder roll-up). Ignored test (undo of create / copy to the Recycle Bin) run once, passed. Workspace total 48 pass + 2 ignored.
- T3 click test 2026-10-17 (git repo + `CARBON_TEST_MARKS`): markers (U, A, M, errors "2", warnings "3", bot icon, folder dots, colored names; branch in status bar), compact `deep/a/b`, drag `notes.txt` onto `docs` (moved, git updated live), filter "rs" (matches + folders), Down in filter, Escape closes, menu Copy + Paste twice (`main copy.rs`), Undo x3 (copies to Recycle Bin, move back, "Undid ..." notice), Cut (faded) + Paste onto compact row (into `deep/a/b`, open buffer followed). Fixed during test: whole list lit up while dragging over a row.
- Not checked: multi-select and all modifier shortcuts (Ctrl/Shift click, Shift+arrows, Ctrl+A/C/X/V/Z, Ctrl-drag copy; synthetic input cannot hold modifiers), drops from Windows Explorer (OLE drag not scriptable here), keyboard shortcuts with modifiers (Shift+Alt+C/R, Ctrl+Shift+Alt+C; synthetic keys cannot hold modifiers), mouse-wheel scroll (synthetic wheel messages ignored by GPUI; list uses GPUI's standard scroll), double-click, macOS/Linux watchers.

## Limits

- Editing `.gitignore` re-dims only the folders it lists again (loaded subfolders keep old flags until they refresh).
- Symlinked folders are leaves (not expandable), so the tree cannot leave the workspace.
- Watch failure is silent in the UI (`Core::watch_error`; tree still works without live updates).
- Not built: file nesting (`Cargo.lock` under `Cargo.toml`), redo, a UI setting for compact folders, highlight of the matched text in the filter, auto-scroll while dragging near the edge.
- Problem and agent markers have no source yet (API only; test hook in debug builds).
- Git status lists untracked files one by one: a huge untracked folder (not ignored) costs time per run; runs are off the UI thread and at most once per second.
- Filter searches only opened folders (no whole-project index yet).
- Explorer clipboard is internal: Ctrl+V does not paste files copied in Windows Explorer, and Ctrl+C does not put files on the OS clipboard.
- Delete of a folder with open, unsaved buffers does not warn (buffers stay open, nothing is lost).
