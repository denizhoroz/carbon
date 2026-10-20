# Workflow system (Phase 0-2)

Built 2026-10-20. Design and later phases: `../not_built/workflow-system.md` (s5.1 and s5.2 list where the build differs from the plan). A workflow is a switchable app mode, one Section Bar icon each (`section-bar.md`), described by a `workflow.toml` manifest. Built-ins: Editor (real), Paradigm Editor and Agent Automation (placeholders). Users add their own workflows and patch built-ins from a folder; no dialog yet (Manage Workflows content is Phase 3).

## Where files live

Three locations (user decision 2026-10-20, Q15-Q16; built the same day). Users will change them in the installer or Settings (not built); until then each has an environment variable.

| Location | Default | Override | Contents |
|---|---|---|---|
| Shipped packages (Carbon's own, read-only) | `carbon-workflow/` next to the binary, else the repo's `apps/carbon-workflow/` (development) | `CARBON_SHIPPED_WORKFLOWS_DIR` | `editor/`, `paradigm/`, `agent/`, each `workflow.toml` (+ assets later) |
| Installed packages (downloaded) | `<OS local data dir>/Carbon/workflows/installed/` (Windows `%LOCALAPPDATA%`) | `CARBON_INSTALLED_WORKFLOWS_DIR`; under `CARBON_WORKFLOWS_DIR` when only that is set | `<id>/<version>/workflow.toml`; the highest semver folder wins. Nothing installs them yet (Phase 4) |
| Preferences and workflow list | `<OS config dir>/Carbon/workflows/` (Windows `%APPDATA%`) | `CARBON_WORKFLOWS_DIR` | Table below |

The Editor manifest is also compiled into carbon-core (`include_str!` of the shipped file): when the shipped folder is missing or has no Editor, Carbon opens with that copy only and a notification says so. Release packaging must copy `apps/carbon-workflow/` to `carbon-workflow/` next to the binary (no installer yet).

Preferences root:

| Path | Layer | Contents |
|---|---|---|
| (shipped) `carbon-workflow/<name>/workflow.toml` | L0 Carbon's own | `carbon.editor`, `carbon.paradigm-editor`, `carbon.agent-automation`; changed only through an overlay |
| `user/<folder>/workflow.toml` | L2 user | A whole workflow, by hand (Duplicate comes in Phase 3) |
| `overlay/<id>.toml` | L3 overlay | Patch of a built-in; delete the file to reset |
| `state/state.json` | L4 state | Active workflow, each workflow's last layout + fingerprint of the default it came from, `disabled`, `quarantined` (id -> reason) |
| `state/entering` | crash guard | Id of the workflow being opened; deleted 2 s after it opened |

`target/carbon-gui/layout.json` (Phase 1) is read once to migrate when `state.json` does not exist, and never written again, so older builds keep reading it. Named layouts (`layouts.json`) are still global (Phase 3).

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

[[commands]]                   # palette commands while this workflow is active
id = "new-file"
label = "New Review File"
run = "carbon::NewFile"        # an existing action; `args = { ... }` for actions that take JSON
[[menus]]                      # optional: also at the end of File / Edit / View / Help
menu = "File"
command = "new-file"
```

Panel kinds: `editor`, `explorer`, `terminal`, `chat`, `placeholder` (needs `title`). Another kind loads as a "Panel `x` is not available" placeholder (warning, not an error). Unknown top-level keys warn; reserved keys (`keybindings`, `requires`, `capabilities`, `[agent]`, ...) are ignored; unknown keys inside `layout`, `panels`, `commands`, `menus` reject the file. A leading UTF-8 BOM is fine.

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
| Remove / turn off (rules for Phase 3) | `Registry::removable`: user and installed only (Carbon's own folders are read-only). `can_turn_off`: all but the Editor; `block` never blocks the Editor |
| Switch workflow | Current layout kept in memory (with its default's fingerprint), target's saved layout loads, else its default; `state.json` written 500 ms after changes |
| Default layout changed since a layout was saved (fingerprint differs) | That workflow's layout resets to the new default, with a notification. No merge yet; `base_panels` is saved for a later merge |
| Broken saved layout for one workflow | Only that workflow resets (notification) |
| `state.json` unreadable | Moved to `state.json.bad`; all workflows start from defaults (notification) |
| Carbon stops within 2 s of opening a user workflow | Next start: that workflow is quarantined (`state.json`), not offered, notification says why. Re-enable: remove it from `quarantined` (dialog in Phase 3) |
| Carbon stops within 2 s of opening a built-in (also at start) | Next start runs in safe mode with a notification; built-ins are never quarantined |
| `--safe-mode` (or the case above) | Built-ins only, no overlays, default layouts, nothing read from or written to the folder; the crash mark is cleared |
| Active workflow hidden, disabled or gone | The Editor (or the first available) opens |
| Workflow commands | Built from action names at menu refresh; one whose action does not exist is left out and logged once. Placed items appear after a separator at the end of their menu; unplaced ones are palette-only under the workflow's name. A command reusing an existing action shows in both places |

## Code

| File | Role |
|---|---|
| `apps/carbon-core/src/workflow.rs` | `WorkflowId`, `WorkflowDef` (parse, `validate`, `layout_fingerprint`, `base_panels`, `unavailable_kinds`), `CommandDef`, `MenuDef`, `Registry` (`builtin`, `load`, `apply_overlay`, `block`, `available`, `resolve`, `default_workflow`, `origin`), `LayoutFile` (Phase 1 `layout.json`, read for migration) |
| `apps/carbon-workflow/{editor,paradigm,agent}/workflow.toml` | Carbon's own workflow packages (data, not a crate) |
| `apps/carbon-core/src/workflow/{overlay,dirs,state}.rs` | `Overlay`/`RegionPatch` + `apply`; `shipped_dir`, `WorkflowDirs` (preferences + installed roots, discovery, newest installed version, `remove_overlay`); `StateFile` (atomic save, `set_aside`), `SavedWorkflow`, `start_entering` / `finish_entering` / `take_unfinished` |
| `apps/carbon-gui/src/workflows.rs` | `init` at start (crash guard, safe mode, state, registry, notices), `registry`, `dirs`, `take_state`, `icon`, `commands` -> `WorkflowCommand` |
| `apps/carbon-gui/src/workbench.rs` | `state: StateFile`, `enter` (crash mark + saved or default layout), `park_current`, `load_saved` (fingerprint, broken entry), `save`, `restore`, `migrate_layout_file` |
| `apps/carbon-gui/src/layouts.rs` | `PanelKind` (unknown kind -> placeholder), `LayoutSpec`; only `Preset::Code` left |
| `apps/carbon-gui/src/menus.rs`, `commands.rs` | Workflow items appended per menu; palette-only workflow commands |
| `apps/carbon-gui/src/main.rs` | `workflows::init(--safe-mode)`, start-up notifications, active workflow's commands in `MenuState` |

## Verified (2026-10-20)

- `cargo test --workspace`: 146 pass (core `tests/workflow.rs` 14, `tests/workflow_layers.rs` 11: user folder, rejects, collisions, overlays apply/reject/reset, notices, block/hidden/default, fingerprint, state round trip and corrupt file, crash mark, BOM; GUI: Editor manifest == `Preset::Code`, placeholder layouts, unknown kind placeholder, workflow commands in menus and palette once). Clippy `-D warnings` and fmt clean.
- Click-tested with `CARBON_WORKFLOWS_DIR` in a scratch folder (separate target dir; `target/carbon-gui` JSON backed up, compared, two empty test chat files removed): five icons (user Review with `git-branch`, generic icon for Sheets); overlay renamed Paradigm; Review layout (editor + chat center, terminal below); Sheets shows "Panel `spreadsheet` is not available"; File menu shows "New Review File" only in Review; `state.json` written, `layout.json` byte-identical; kill within 2 s of opening Review -> quarantined with notification; kill within 2 s of start (Editor) -> safe mode notice, built-ins only, `state.json` unchanged. Several start-up notifications stack (the kit expands them on hover).
- Packages (same day): 150 workspace tests (new: shipped folders load as `Builtin`, compiled-in Editor fallback with and without a partial folder, newest installed version, `carbon.*` rejected for installed, overlay on installed, removable / can-turn-off rules, Editor never blocked). Click-tested: repo `apps/carbon-workflow` gives the three icons; `CARBON_SHIPPED_WORKFLOWS_DIR` on an empty folder gives the Editor only plus a notification.
- Not checked by hand: palette entries of workflow commands, overlay `hidden`/`order` in the bar (core-tested), fingerprint reset notice, macOS/Linux config dir.

## Limits

- No UI to enable, disable, re-enable a quarantined workflow, duplicate, reset or remove (Phase 3 Manage Workflows); edit files by hand.
- Layout merge after a default changes is a reset, not a merge.
- `multi` / `closable` are parsed but not enforced; no scoped key bindings (Q9); `requires_workspace` stored only.
- A placeholder for an unknown kind is saved as a plain placeholder; when the kind later exists, that workflow keeps the placeholder until Reset Layout.
- The crash mark covers 2 s after opening; a crash later is not tied to a workflow. Killing Carbon within 2 s of start (e.g. while debugging) triggers safe mode once.
- Registry is loaded once at start; file changes need a restart.
