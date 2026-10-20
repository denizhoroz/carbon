# Section Bar

Built 2026-10-16. User decision: a vertical bar like VS Code's activity bar, called the **Section Bar**; its icons switch the whole app to a different section, each with its own layout. In user-facing terms a section is a **workflow** (2026-10-19). Since 2026-10-20 the bar lists the available workflows of the workflow registry (`workflows.md`): Editor (real), Paradigm Editor and Agent Automation (placeholders), plus user workflows from the workflow folder; hidden, disabled and quarantined ones are left out.

## What

| Part | Behaviour |
|---|---|
| Bar | Fixed 52 px strip left of the dock area, under the title bar; theme `title_bar` colours, right border; 8 px top padding |
| Icons | One ghost icon button per workflow, in manifest `order`; 36 px square (8 px clear each side), 20 px icon (manifest `icon` = Lucide name if the app embeds it, else `layout-dashboard`); active one shows selected; tooltip + accessibility label = workflow name |
| Switching | Click icon or View > Section (checkmark; also in the palette). Current workflow's layout is kept in memory, target's last layout loads (its manifest default the first time) |
| Bottom group | Pinned to the bar's bottom, top to bottom: Manage Extensions (Lucide `puzzle`), Manage Workflows (`workflow`), Manage Settings (`settings`). Same button style, never selected; tooltip = dialog title = that name; each opens its dialog (720 px wide, placeholder body: icon, "Nothing here yet."). No content yet. Planned: Extensions = official + user-added plugins, VS Code-like (compilers, linters, appearances); Workflows = add/remove workflows (built-in + custom); Settings = all user options (system, appearance, agents, ...) |
| Persistence | `<config>/Carbon/workflows/state/state.json` since Phase 2 (`workflows.md`); restart reopens the active workflow with its layout. The Phase 1 `target/carbon-gui/layout.json` is migrated once and left untouched (older builds still read it) |
| Reset Layout | Resets the active workflow to its manifest default; docks the manifest does not use are emptied and closed |
| Safe mode | `carbon-gui --safe-mode [folder]`, or automatic after Carbon stopped while opening a built-in: built-ins only, default layouts, nothing saved (`workflows.md`) |

## Workflows

| Workflow (id) | Icon | Default layout | Status |
|---|---|---|---|
| Editor (`carbon.editor`, legacy `code`) | `code` ("< >") | Left: Explorer; center: Editor; right: Chat; bottom: Terminal (= `Preset::Code`, golden test) | Real panels |
| Paradigm Editor (`carbon.paradigm-editor`) | `network` | Center: placeholder "Paradigm Editor"; docks empty and closed | Placeholder (`TestPanel`) |
| Agent Automation (`carbon.agent-automation`) | `bot` | Center: placeholder "Agent Automation"; docks empty and closed | Placeholder (`TestPanel`) |
| User workflows | manifest `icon` | manifest `[layout]` | `workflows.md` |

Adding one of Carbon's own workflows: a folder `apps/carbon-workflow/<name>/` with `workflow.toml` (shipped next to the binary); its icon in `icon_assets!` (`app_assets.rs`) if not in the kit default set. Manifest format, user folder and overlays: `workflows.md`.

## Code

| File | Role |
|---|---|
| `apps/carbon-core/src/workflow.rs` (+ `workflow/`) | Registry, manifests, overlays, state (`workflows.md`) |
| `apps/carbon-gui/src/workflows.rs` | Registry loaded at start, icons, workflow commands (`workflows.md`) |
| `apps/carbon-gui/src/section_bar.rs` | `SectionBar` view over `registry().available()`, dispatches `SwitchSection { id }`; bottom Manage buttons |
| `apps/carbon-gui/src/manage.rs` | `Manage` enum (Extensions, Workflows, Settings), `open` = placeholder dialog |
| `apps/carbon-gui/src/workbench.rs` | `switch_section`, `enter`, `apply_workflow_default`, `reset_layout`, `state.json` save/restore |
| `apps/carbon-gui/src/layouts.rs` | `PanelKind` (kind -> panel), `LayoutSpec` (`of_workflow`, `code`, `build`); `Preset::Code` builds `LayoutSpec::code()` |
| `apps/carbon-gui/src/menus.rs` | `SwitchSection { id: WorkflowId }`, View > Section from `registry().available()` |
| `apps/carbon-gui/src/main.rs` | Row: Section Bar + workbench; keeps the bar's active icon in sync; `--safe-mode` flag |

## Verified

- Builds clean; tests pass; app starts and writes `layout.json` with `active: code` and panels Explorer, Editor, Chat, Terminal.
- 2026-10-20 (Phase 0-1): `cargo test --workspace` green (core `tests/workflow.rs` 14: built-ins, validation, rejects, old/new `layout.json` incl. an old-build reader; GUI 3 new: Editor manifest == `Preset::Code`, placeholder layout, built-ins load); clippy `-D warnings` and fmt clean. Click-tested (test build in a separate target dir, state files backed up and restored): user's old `layout.json` restores into the Editor; three icons render with selection; Paradigm Editor and Agent Automation show their placeholder with docks closed; back to Editor restores its layout; restart reopens Paradigm Editor; `--safe-mode` opens defaults and leaves `layout.json` byte-identical.
- Not checked by hand: tooltips, View > Section menu, palette entries.
- 2026-10-20 (Phase 2): see `workflows.md` (user workflows in the bar, quarantine, safe mode).

## Limits

- Old single-layout `layout.json` (T1/T2 format) is ignored on first start and replaced by the workflow default.
- Named layouts and presets are global, not per workflow (per-workflow named layouts: Phase 3); applying one replaces the active workflow's layout.
- Presets Paradigm and Agents were removed 2026-10-20 (user): the workflows replace them; View > Layout > Preset has Code only.
- No Manage Workflows content yet (Phase 3); no scroll arrows when workflows overflow the bar (Phase 3, s4.10).
- Manage dialogs are empty placeholders (added 2026-10-19); no menu entry or shortcut opens them.
- No keyboard shortcut for sections yet.
