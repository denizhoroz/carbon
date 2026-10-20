# Workflow system (Phase 0-3, views, webview panels)

Built 2026-10-20 (Phase 0-3) and 2026-10-21 (package files, declarative views, webview panels). What is left, its design and the decisions: `../not_built/workflow-system.md` (s1 To do; s4.4 webview research). A workflow is a switchable app mode, one Section Bar icon each (`section-bar.md`), described by a `workflow.toml` manifest. Built-ins: Editor (real), Paradigm Editor and Agent Automation (placeholders). Users add their own workflows and patch built-ins from a folder, and manage them in the Manage Workflows dialog (Phase 3, below).

## Where files live

Three locations (user decision 2026-10-20, Q15-Q16; built the same day). Users will change them in the installer or Settings (not built); until then each has an environment variable.

| Location | Default | Override | Contents |
|---|---|---|---|
| Shipped packages (Carbon's own, read-only) | `carbon-workflow/` next to the binary, else the repo's `apps/carbon-workflow/` (development) | `CARBON_SHIPPED_WORKFLOWS_DIR` | `editor/`, `paradigm/`, `agent/`, each a package: `workflow.toml` at its root plus any files it names (Package files below) |
| Installed packages (downloaded) | `<OS local data dir>/Carbon/workflows/installed/` (Windows `%LOCALAPPDATA%`) | `CARBON_INSTALLED_WORKFLOWS_DIR`; under `CARBON_WORKFLOWS_DIR` when only that is set | `<id>/<version>/workflow.toml`; the highest semver folder wins. Nothing installs them yet (Phase 4) |
| Preferences and workflow list | `<OS config dir>/Carbon/workflows/` (Windows `%APPDATA%`) | `CARBON_WORKFLOWS_DIR` | Table below |

The Editor manifest is also compiled into carbon-core (`include_str!` of the shipped file): when the shipped folder is missing or has no Editor, Carbon opens with that copy only and a notification says so. Release packaging must copy `apps/carbon-workflow/` to `carbon-workflow/` next to the binary (no installer yet).

Preferences root:

| Path | Layer | Contents |
|---|---|---|
| (shipped) `carbon-workflow/<name>/workflow.toml` | L0 Carbon's own | `carbon.editor`, `carbon.paradigm-editor`, `carbon.agent-automation`; changed only through an overlay |
| `user/<folder>/workflow.toml` | L2 user | A whole workflow, by hand or from Manage Workflows > Duplicate |
| `overlay/<id>.toml` | L3 overlay | Patch of a built-in; delete the file to reset |
| `state/state.json` | L4 state | Active workflow, each workflow's last layout + fingerprint of the default it came from, `disabled`, `quarantined` (id -> reason) |
| `state/entering` | crash guard | Id of the workflow being opened; deleted 2 s after it opened |

`target/carbon-gui/layout.json` (Phase 1) is read once to migrate when `state.json` does not exist, and never written again, so older builds keep reading it. Named layouts and auto-hide flags are per workflow since Phase 3 (`state.json` `layouts`); the old global `target/carbon-gui/layouts.json` becomes the Editor's set once and is not written again.

## Manifest (schema 1)

```toml
id = "me.review"               # publisher.name; carbon.* is reserved
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"     # host contract 0.1.0
name = "Review"
icon = "git-branch"            # Lucide name; only icons the app embeds, else a generic one
order = 150                    # Section Bar position (Editor 100, Paradigm 200, Agents 300)
hidden = false

[layout.center]                # center needs at least one panel
panels = ["editor", "chat"]
[layout.bottom]                # left/right/bottom optional; unset = empty and closed
panels = ["terminal"]
open = true
size = 200                     # optional, pixels

[panels]                       # names used above; kind defaults to the name
editor = { multi = true }
chat = {}
terminal = {}
notes = { kind = "placeholder", title = "Notes" }
home = { kind = "view", title = "Home", view = "home" }          # declarative view (below)
web = { kind = "webview", title = "Web", page = "ui/index.html" } # webview (below, experimental)

[[commands]]                   # palette commands while this workflow is active
id = "new-file"
label = "New Review File"
run = "carbon::NewFile"        # an existing action; `args = { ... }` for actions that take JSON
[[menus]]                      # optional: also at the end of File / Edit / View / Help
menu = "File"
command = "new-file"

[[views.home]]                 # declarative view `home`: a list of widgets
type = "heading"
text = "Review"
```

Panel kinds: `editor`, `explorer`, `terminal`, `chat`, `placeholder` (needs `title`), `view` (needs `view`), and `webview` (needs a build with the `webview` cargo feature; `page` optional). Another kind loads as a "Panel `x` is not available" placeholder (warning, not an error). Unknown top-level keys warn; reserved keys (`keybindings`, `requires`, `capabilities`, `[agent]`, ...) are ignored; unknown keys inside `layout`, `panels`, `commands`, `menus` reject the file. A leading UTF-8 BOM is fine.

## Package files (Q22, 2026-10-21)

A package is its folder; only `workflow.toml` at the root is fixed, every other file is named by a path in the manifest (today: a webview `page`). Carbon never looks for files by folder name. Rules, checked when the manifest loads (`check_package_path`) and again when a file is read (`resolve_package_file`):

| Rule | Why |
|---|---|
| Relative, `/` separators; no `.`, `..`, `//`, absolute or drive paths | The same reference works everywhere and cannot leave the package |
| Each name matches the file on disk exactly, case included | Works on case-sensitive Linux too |
| No symbolic links followed | A link could point outside the package |
| No Windows reserved names (`CON`, `NUL`, `COM1`, ...), no `< > : " \ | ? *`, no name ending in `.` or space | Installs on Windows |
| At most 160 bytes | Windows path length, with room for the install folders |

`Registry::package_dir(id)` is each workflow's folder (the manifest's folder for user and installed workflows, `carbon-workflow/<name>/` for shipped ones; none for the compiled-in Editor).

## Declarative views (tier 1, 2026-10-21)

A `view` panel draws a widget list from the manifest's `[[views.<name>]]` with Carbon's own widgets; no code runs. Every build, every platform.

| Widget | Fields |
|---|---|
| `heading` | `text` |
| `text` | `text`, `muted` |
| `markdown` | `text` (links open in the system browser) |
| `button` | `label`, `command`, `icon` (embedded icon name), `style` = `secondary` (default) / `primary` / `ghost` |
| `row` / `column` | `children`; `column` also `align` = `start` / `center` (a view that is one centered column is centered vertically: empty states) |
| `divider` | none |
| `list` | `items`: `label`, `description`, `icon`, `command` (rows with a command are clickable) |

Buttons and list rows run the workflow's `[[commands]]`, so a view can do only what a command can. Load-time checks reject unknown widget types or fields, commands not in `[[commands]]`, nesting over 8 levels, more than 500 widgets in one view, a `view` panel without an existing `view`, and `view` on another kind. A command whose action this Carbon lacks shows "Command `x` is not available" when clicked. The view is read at each render: Manage Workflows > Reload shows manifest edits without a restart.

## Webview panels (experimental, `webview` cargo feature, 2026-10-21)

Off by default: `cargo run -p carbon-gui --features webview`. Windows and macOS only (the kit's `gpui-wry` 0.7.3 over `lb-wry` 0.53.3); no Linux webview yet. On Windows the feature sets `GPUI_DISABLE_DIRECT_COMPOSITION` at start, which switches GPUI to the classic window swap chain for the whole app (needed for a child webview window; no visible difference seen, speed not measured).

| Topic | Behaviour |
|---|---|
| Page | `page` = a package file, served by the custom protocol `carbon-page`: one origin per workflow (host = id with `.` written `--`; WebView2 shows `http://carbon-page.<host>`), `GET` only, files only through `resolve_package_file`. Without `page`: a built-in test page |
| Limits on the page | CSP: own files plus inline/data/blob content, no network (waits for the `net` capability, Q19); navigation only inside its origin; `window.open` refused |
| Bridge | Page to Carbon: `window.carbon.send({...})` (IPC, JSON, max 64 KB, known types only: `ping`, `focus`, `key`). Carbon to page: `window.carbon.onMessage`. Theme colours as CSS variables (`--carbon-background`, `--carbon-foreground`, `--carbon-muted-foreground`, `--carbon-border`, `--carbon-primary`, `--carbon-primary-foreground`), updated when the theme changes |
| Shortcuts | While the page has focus, Carbon's reserved keys (the terminal's `app_keeps` list: palette, dock toggles, quit, saved layouts) are caught by an injected script and run in Carbon; other keys stay in the page |
| Covering | The page is a native window above GPUI, so it is hidden when its tab is in the background, its dock is closed, another group is zoomed, or a dialog, the palette, a title-bar menu or a tab menu holds focus. Tooltips and notifications can be covered |
| Lifetime | Built after the panel's first render inside the panel's task (WebView2 creation runs the message loop; inside an update it panicked), hidden until loaded, kept while the panel moves between docks (no reload), restored after restart (`workflow` + `page` in the saved layout) |

## Overlay

```toml
# overlay/carbon.editor.toml
name = "Code"
order = 50
hidden = false
[layout.right]
remove = ["chat"]              # or panels = [...] to replace; add = [...]
open = false
[layout.bottom]
add = ["chat2"]
[panels]
chat2 = { kind = "chat" }      # new names only; existing ones cannot be redefined
```

Only Carbon's own and installed workflows take overlays (a user workflow is edited directly). `id`, `capabilities`, `requires` or any other key is an error. A bad overlay is rejected and the built-in keeps its base definition; a `remove` of a panel that is not there is a logged notice.

## Behaviour

| Situation | What happens |
|---|---|
| Bad manifest or overlay | Skipped with reasons in the log (stderr); one notification lists the files; every other workflow loads |
| Duplicate id or legacy id, installed or user file using `carbon.*` | The later file is rejected (load order: shipped, installed, user) |
| Remove / turn off | `Registry::removable`: user and installed only (Carbon's own folders are read-only). `can_turn_off`: all but the Editor; `block` never blocks the Editor |
| Switch workflow | Current layout kept in memory (with its default's fingerprint), target's saved layout loads, else its default; `state.json` written 500 ms after changes |
| Default layout changed since a layout was saved (fingerprint differs) | That workflow's layout resets to the new default, with a notification. No merge yet; `base_panels` is saved for a later merge |
| Broken saved layout for one workflow | Only that workflow resets (notification) |
| `state.json` unreadable | Moved to `state.json.bad`; all workflows start from defaults (notification) |
| Carbon stops within 2 s of opening a user workflow | Next start: that workflow is quarantined (`state.json`), not offered, notification says why. Re-enable: Manage Workflows > Turn On |
| Carbon stops within 2 s of opening a built-in (also at start) | Next start runs in safe mode with a notification; built-ins are never quarantined |
| `--safe-mode` (or the case above) | Built-ins only, no overlays, default layouts, nothing read from or written to the folder; the crash mark is cleared |
| Active workflow hidden, disabled or gone | The Editor (or the first available) opens |
| Workflow commands | Built from action names at menu refresh; one whose action does not exist is left out and logged once. Placed items appear after a separator at the end of their menu; unplaced ones are palette-only under the workflow's name. A command reusing an existing action shows in both places |

## Manage Workflows dialog (Phase 3)

Section Bar > Manage Workflows. One row per loaded workflow (hidden, off and quarantined ones too): icon, name, origin tag (Carbon / Installed / User), status tag (On / Off / Hidden / Turned off after a crash, with the reason), "Changed" when an overlay applies, id and version. Below the rows: files that did not load, with their problems. Footer: Open Folder (the preferences folder; creates `user/`), Reload (reads the files again, no restart), Close.

| Action | Shown for | Does |
|---|---|---|
| Turn Off / Turn On | all but the Editor | `state.json` `disabled`; Turn On also clears a quarantine. Turning off the active workflow switches to the default |
| Duplicate | all | Copies the effective workflow (overlay applied) to `user/<name>/workflow.toml`: id `user.<slug>` (`-2`, ... when taken), name "<name> (copy)", order + 1, `derived_from = { id, version }`, no legacy ids. Notification on success |
| Reset | workflows with an overlay or a saved layout | Confirms, then deletes the overlay and the saved dock layout; named layouts stay. The active workflow switches to its default at once |
| Remove | user and installed workflows | Confirms, then deletes the folder (user) or the package with all versions (installed). Paths are checked to lie inside the user / installed folder. A user workflow's saved state goes too; an installed one's stays for a reinstall |

Safe mode shows a note and disables the actions. Hide/show is not in the dialog (Turn Off covers it); `hidden = true` in an overlay or manifest still works.

## Section Bar overflow (Phase 3)

When the workflows do not fit, the list scrolls (mouse wheel) and up/down arrows appear at its ends (disabled at the top / bottom); each click moves one button. The Manage buttons stay at the bottom.

## Code

| File | Role |
|---|---|
| `apps/carbon-core/src/workflow.rs` | `WorkflowId`, `WorkflowDef` (parse, `validate`, `layout_fingerprint`, `base_panels`, `unavailable_kinds`), `CommandDef`, `MenuDef`, `Registry` (`builtin`, `load`, `apply_overlay`, `block`, `available`, `resolve`, `default_workflow`, `origin`), `LayoutFile` (Phase 1 `layout.json`, read for migration) |
| `apps/carbon-workflow/{editor,paradigm,agent}/workflow.toml` | Carbon's own workflow packages (data, not a crate) |
| `apps/carbon-core/src/workflow/{overlay,dirs,state}.rs` | `Overlay`/`RegionPatch` + `apply`; `shipped_dir`, `WorkflowDirs` (preferences + installed roots, discovery, newest installed version, `remove_overlay`); `StateFile` (atomic save, `set_aside`), `SavedWorkflow`, `start_entering` / `finish_entering` / `take_unfinished` |
| `apps/carbon-gui/src/workflows.rs` | `init` at start (crash guard, safe mode, state, registry, notices), `registry` (an `Arc` snapshot), `reload`, `dirs`, `take_state`, `icon`, `commands` -> `WorkflowCommand` |
| `apps/carbon-gui/src/workbench.rs` | `state: StateFile`, `enter` (crash mark + saved or default layout + auto-hide), `park_current` (dock + named layouts), `store_for`, `load_saved` (fingerprint, broken entry), `save`, `restore`, `migrate_layout_file`; dialog actions `set_workflow_on`, `duplicate_workflow`, `reset_workflow`, `remove_workflow`, `reload_workflows`, `workflow_flags`, `has_saved_layout` |
| `apps/carbon-gui/src/manage_workflows.rs` | Manage Workflows dialog (`WorkflowsView`, confirm dialog, Open Folder) |
| `apps/carbon-gui/src/section_bar.rs` | Scrolling workflow list with arrows (`ScrollHandle`) |
| `apps/carbon-gui/src/layouts.rs` | `PanelKind` (unknown kind -> placeholder), `LayoutSpec`; only `Preset::Code` left |
| `apps/carbon-gui/src/menus.rs`, `commands.rs` | Workflow items appended per menu; palette-only workflow commands |
| `apps/carbon-gui/src/main.rs` | `workflows::init(--safe-mode)`, start-up notifications, active workflow's commands in `MenuState` |
| `apps/carbon-core/src/workflow/{package,view}.rs` | `check_package_path`, `resolve_package_file`; `Widget`, `ListItem`, `check_view` (limits) |
| `apps/carbon-gui/src/view_panel.rs` | `ViewPanel`: draws a view, runs commands (`workflows::command_action`), saved as `workflow` + `view` |
| `apps/carbon-gui/src/webview_panel.rs`, `webview_panel/protocol.rs`, `webview_panel/spike.html` | `WebviewPanel` (feature `webview`): build, IPC, theme, keys, show/hide; `carbon-page` protocol, CSP, navigation rule; test page |
| `apps/carbon-gui/src/workbench.rs` `sync_webviews` | Every frame: which webview pages may show (dock open, no zoom elsewhere, no overlay) |

## Verified (2026-10-20)

- `cargo test --workspace`: 146 pass (core `tests/workflow.rs` 14, `tests/workflow_layers.rs` 11: user folder, rejects, collisions, overlays apply/reject/reset, notices, block/hidden/default, fingerprint, state round trip and corrupt file, crash mark, BOM; GUI: Editor manifest == `Preset::Code`, placeholder layouts, unknown kind placeholder, workflow commands in menus and palette once). Clippy `-D warnings` and fmt clean.
- Click-tested with `CARBON_WORKFLOWS_DIR` in a scratch folder (separate target dir; `target/carbon-gui` JSON backed up, compared, two empty test chat files removed): five icons (user Review with `git-branch`, generic icon for Sheets); overlay renamed Paradigm; Review layout (editor + chat center, terminal below); Sheets shows "Panel `spreadsheet` is not available"; File menu shows "New Review File" only in Review; `state.json` written, `layout.json` byte-identical; kill within 2 s of opening Review -> quarantined with notification; kill within 2 s of start (Editor) -> safe mode notice, built-ins only, `state.json` unchanged. Several start-up notifications stack (the kit expands them on hover).
- Packages (same day): 150 workspace tests (new: shipped folders load as `Builtin`, compiled-in Editor fallback with and without a partial folder, newest installed version, `carbon.*` rejected for installed, overlay on installed, removable / can-turn-off rules, Editor never blocked). Click-tested: repo `apps/carbon-workflow` gives the three icons; `CARBON_SHIPPED_WORKFLOWS_DIR` on an empty folder gives the Editor only plus a notification.
- Not checked by hand: palette entries of workflow commands, overlay `hidden`/`order` in the bar (core-tested), fingerprint reset notice, macOS/Linux config dir.

- Phase 3 (same day): 155 workspace tests (new: manifest -> TOML -> manifest round trip for Duplicate, free user ids, create folder `-2` suffix, remove user/installed with path checks (never the user folder itself or anything outside), named layouts in `state.json`, `LayoutStore` as a state value). Click-tested with 16 user workflows in a scratch folder: arrows appear, step down to the end (down disabled, up enabled); dialog lists all rows and the broken file; Turn Off on Paradigm Editor (tag Off, gone from the bar, still off after restart); Duplicate of the Editor (folder `user/editor-copy`, row "Editor (copy)", notification); Remove with confirm (folder deleted, row gone); the Editor got the old global named layouts. Not clicked: Reset, Turn On after a quarantine, Reload, Open Folder, per-workflow named layouts via the Save dialog (unit-tested).

- Package files, views, webviews (2026-10-21): 166 workspace tests (new: path rules, exact-case and outside-package resolving, `page` and `view` rules, package folders, view parsing/validation/limits/round trip, view panel layout, reserved keys, page messages, page origins). Clippy and fmt clean with and without `webview` (44 GUI tests with it). Click-tested in a scratch folder: a View Demo workflow (every widget; New File, Palette and a list row run; a missing action shows the notice; restart restores; Reload shows an edited heading); a Web Spike workflow (package page with its JS/CSS/JSON, network blocked, link away and `window.open` blocked, two webviews at once, palette/menus/dialog/tab menu hide pages, zoom (bug found and fixed), move to the bottom dock without reload, restart, dock close and window resize follow). Real keypresses in a page (F1, Ctrl+P, Ctrl+Shift+P, Ctrl+B) checked by the user.

## Limits

- No hide/show in the dialog; no Repair/Remove buttons on a "not available" placeholder; no diff of a copy against its parent (`derived_from` is stored).
- Layout merge after a default changes is a reset, not a merge.
- `multi` / `closable` are parsed but not enforced; no scoped key bindings (Q9); `requires_workspace` stored only.
- A placeholder for an unknown kind is saved as a plain placeholder; when the kind later exists, that workflow keeps the placeholder until Reset Layout.
- The crash mark covers 2 s after opening; a crash later is not tied to a workflow. Killing Carbon within 2 s of start (e.g. while debugging) triggers safe mode once.
- Registry is loaded at start and on Manage Workflows > Reload; nothing watches the folders.
- Duplicate copies only `workflow.toml`, so a duplicated workflow loses its package files (a webview page).
- Views have no state: no inputs, toggles, forms, tables or trees (tier 2 or host data later).
- Webview: off by default, Windows/macOS only; a tab click does not focus the panel (a click in the page does); no page-to-Carbon API beyond the test messages; workflow ids `a-.b` and `a.-b` (rare) map to the same page origin; not tested: auto-hide docks over a page, flicker during an edge drag, memory per page.
