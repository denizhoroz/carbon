# Workspace folders: open, create, close

Built 2026-10-16. User request: create or load folders (workspaces) from the menu bar, the status bar workspace button, and Create/Open buttons in the Explorer.

## Entry points (all dispatch the same actions)

| Where | Open | Create | Close |
|---|---|---|---|
| File menu | Open Folder... (Ctrl+K Ctrl+O) | Create Folder... | Close Folder (disabled without a folder) |
| Status bar workspace button | Menu opens upward: Open Folder... | Create Folder... | Close Folder (only with a folder) |
| Explorer empty state (no folder) | Open Folder (primary) | Create Folder | - |
| Command line | `carbon-gui <folder>` | - | - |

## Behaviour

| Action | Flow |
|---|---|
| `OpenFolder` | Native folder picker (`cx.prompt_for_paths`, directories only) -> `Core::open_workspace`; error shown as a notification |
| `CreateFolder` | Dialog: Name (Enter submits) + Location (default: parent of the open folder, else home) with Browse... (native picker) -> `Core::create_workspace` -> opens it; on error the dialog stays open and a notification says why |
| `CloseFolder` | `Core::close_workspace`; open buffers/terminals/agent sessions stay |

Name rules (core `file_ops::validate_name`, shared with Explorer new/rename): trimmed; not empty, `.`, `..`, trailing `.`; no `/ \ : * ? " < > |` or control characters; must not exist yet (no overwrite or reuse).

After any change the `Core` entity is notified: Explorer, status bar (name, branch) and the File menu (Close Folder enabled) update.

## Code

| File | Role |
|---|---|
| `apps/carbon-gui/src/workspace_ops.rs` | `open_folder`, `open_create_dialog` (+ `CreateFolderForm`), `close_folder`; `CurrentCore` global for registry-built panels |
| `apps/carbon-gui/src/explorer_panel.rs` | First real panel: empty state with the two buttons; with a folder: the file tree (`file-tree.md`) |
| `apps/carbon-gui/src/menus.rs` | `CreateFolder`, `CloseFolder` actions; File menu; Ctrl+K Ctrl+O |
| `apps/carbon-gui/src/status_bar.rs` | Workspace item is a ghost button with `dropdown_menu_with_anchor(BottomLeft)` |
| `apps/carbon-gui/src/main.rs` | Action handlers; sets `CurrentCore` before the workbench loads |
| `apps/carbon-gui/src/workbench.rs` | Layout version 2 (Explorer became real); older saved layouts dropped, older named layouts refused with a notice |
| `apps/carbon-core/src/file_ops.rs`, `lib.rs` | `create_folder` (moved from `workspace.rs` 2026-10-17); `Core::create_workspace`, `Core::close_workspace`; errors `InvalidName` ("invalid name"), `AlreadyExists` |

## Verified

- Core tests `tests/create_workspace.rs` (4): create + open, 10 bad names rejected with nothing created, existing name / missing parent refused, close keeps buffers. Workspace total 30 tests pass (27 core + 3 GUI); clippy clean.
- Screenshots: no folder (Explorer empty state, "No folder") and with this repo ("carbon", path, branch "main").
- Click test 2026-10-17 (window messages to the app; native picker driven by its controls). Pass:

| Path | Result |
|---|---|
| Status bar button | Menu opens upward: Open / Create / Close Folder |
| Create dialog: bad name (`a?b`, Enter) / existing name (Create button) | Refused with notification; dialog stays open |
| Create dialog: valid name (Enter, Create button) | Folder made, opened; Explorer + status bar update |
| Create dialog: Browse... | Native picker; folder created in the chosen location |
| Cancel button, Escape | Dialog closes, nothing created |
| Close Folder (status menu) | Explorer empty state, "No folder"; File > Close Folder greyed |
| Explorer Open Folder / File > Open Folder | Native picker opens; OK opens the folder, Cancel keeps the current one |
| Explorer Create Folder | Dialog, location = home without a folder |

- Fixed during the test: dialog closed on error (Enter is the dialog's `Confirm`); Explorer buttons dead after a menu closed (no focus, see STATUS error log); Name input not focused when opened from the Explorer (focus now deferred); location and Explorer path truncated at the end (now `text_ellipsis_start`).
- Not tested: Ctrl+K Ctrl+O (needs real modifier keys).

## Limits

- No recent folders list and the last folder is not reopened on start.
- Layout is still one per section, not per folder.
- `CurrentCore` global assumes one window.
- Menus show only the first key of a chord ("Ctrl+K" for Ctrl+K Ctrl+O): kit `Kbd::from_binding` takes `keystrokes().first()`.
- Old saved layouts (version 1) were dropped once on upgrade; named layouts saved before must be saved again.
