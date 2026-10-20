# Section Bar

Built 2026-10-16. User decision: a vertical bar like VS Code's activity bar, called the **Section Bar**; its icons switch the whole app to a different section, each with its own layout. In user-facing terms a section is a **workflow** (2026-10-19). Since 2026-10-20 the bar lists the workflows of a registry built from `workflow.toml` manifests (workflow system Phase 0-1, `../not_built/workflow-system.md`): Editor (real), Paradigm Editor and Agent Automation (placeholders). User-built workflows come in Phase 2.

## What

| Part | Behaviour |
|---|---|
| Bar | Fixed 52 px strip left of the dock area, under the title bar; theme `title_bar` colours, right border; 8 px top padding |
| Icons | One ghost icon button per workflow, in manifest `order`; 36 px square (8 px clear each side), 20 px icon (manifest `icon` = Lucide name if the app embeds it, else `layout-dashboard`); active one shows selected; tooltip + accessibility label = workflow name |
| Switching | Click icon or View > Section (checkmark; also in the palette). Current workflow's layout is kept in memory, target's last layout loads (its manifest default the first time) |
| Bottom group | Pinned to the bar's bottom, top to bottom: Manage Extensions (Lucide `puzzle`), Manage Workflows (`workflow`), Manage Settings (`settings`). Same button style, never selected; tooltip = dialog title = that name; each opens its dialog (720 px wide, placeholder body: icon, "Nothing here yet."). No content yet. Planned: Extensions = official + user-added plugins, VS Code-like (compilers, linters, appearances); Workflows = add/remove workflows (built-in + custom); Settings = all user options (system, appearance, agents, ...) |
| Persistence | `target/carbon-gui/layout.json` = `{ active, sections, active_workflow?, workflows? }`. Workflows with a legacy id (Editor = `code`) are stored under it in `sections`, and `active` always names one, so builds from before workflows still read the file and keep the Editor layout; other workflows go in `workflows` / `active_workflow`, which old builds ignore. Layouts of ids the registry no longer has are kept. Restart reopens the active workflow with its layout |
| Reset Layout | Resets the active workflow to its manifest default; docks the manifest does not use are emptied and closed |
| Safe mode | `carbon-gui --safe-mode [folder]`: every workflow opens with its default layout; `layout.json` is neither read nor written |

## Workflows

| Workflow (id) | Icon | Default layout | Status |
|---|---|---|---|
| Editor (`carbon.editor`, legacy `code`) | `code` ("< >") | Left: Explorer; center: Editor; right: Chat; bottom: Terminal (= `Preset::Code`, golden test) | Real panels |
| Paradigm Editor (`carbon.paradigm-editor`) | `network` | Center: placeholder "Paradigm Editor"; docks empty and closed | Placeholder (`TestPanel`) |
| Agent Automation (`carbon.agent-automation`) | `bot` | Center: placeholder "Agent Automation"; docks empty and closed | Placeholder (`TestPanel`) |

Adding a built-in workflow: a manifest in `apps/carbon-core/src/workflow/` listed in `BUILTINS` (`workflow.rs`); its icon in `icon_assets!` (`app_assets.rs`) if not in the kit default set. Manifest fields (schema 1): `id`, `schema_version`, `version`, `carbon_api`, `name`, `description`, `icon`, `order`, `legacy_ids`, `requires_workspace` (stored, unused), `[layout.center|left|right|bottom]` (`panels`, `size`, `open`), `[panels]` (`kind`, `title`, `multi`, `closable`; the last two not enforced yet). Panel kinds: `editor`, `explorer`, `terminal`, `chat`, `placeholder` (needs `title`).

## Code

| File | Role |
|---|---|
| `apps/carbon-core/src/workflow.rs` (+ `workflow/*.toml`) | `WorkflowId`, `WorkflowDef` (TOML parse, `validate`), `Registry` (built-ins, rejects bad manifests one by one, `resolve` by id or legacy id), `LayoutFile<S>` (the `layout.json` format above); no UI types. Tests: `tests/workflow.rs` |
| `apps/carbon-gui/src/workflows.rs` | `registry()` (built-ins, problems printed once), `PANEL_KINDS`, `icon`, `SafeMode` global |
| `apps/carbon-gui/src/section_bar.rs` | `SectionBar` view over the registry, dispatches `SwitchSection { id }`; bottom Manage buttons |
| `apps/carbon-gui/src/manage.rs` | `Manage` enum (Extensions, Workflows, Settings), `open` = placeholder dialog |
| `apps/carbon-gui/src/workbench.rs` | `switch_section`, per-workflow layouts, `apply_workflow_default`, save/restore through `LayoutFile`, `reset_layout`, safe mode |
| `apps/carbon-gui/src/layouts.rs` | `PanelKind` (kind -> panel), `LayoutSpec` (`of_workflow`, `code`, `build`); `Preset::Code` builds `LayoutSpec::code()`; other presets still match titles (`tabs`) |
| `apps/carbon-gui/src/menus.rs` | `SwitchSection { id: WorkflowId }`, View > Section from the registry |
| `apps/carbon-gui/src/main.rs` | Row: Section Bar + workbench; keeps the bar's active icon in sync; `--safe-mode` flag |

## Verified

- Builds clean; tests pass; app starts and writes `layout.json` with `active: code` and panels Explorer, Editor, Chat, Terminal.
- 2026-10-20 (Phase 0-1): `cargo test --workspace` green (core `tests/workflow.rs` 14: built-ins, validation, rejects, old/new `layout.json` incl. an old-build reader; GUI 3 new: Editor manifest == `Preset::Code`, placeholder layout, built-ins load); clippy `-D warnings` and fmt clean. Click-tested (test build in a separate target dir, state files backed up and restored): user's old `layout.json` restores into the Editor; three icons render with selection; Paradigm Editor and Agent Automation show their placeholder with docks closed; back to Editor restores its layout; restart reopens Paradigm Editor; `--safe-mode` opens defaults and leaves `layout.json` byte-identical.
- Not checked by hand: tooltips, View > Section menu, palette entries.

## Limits

- Old single-layout `layout.json` (T1/T2 format) is ignored on first start and replaced by the workflow default.
- Named layouts and presets are global, not per workflow (per-workflow named layouts: Phase 3); applying one replaces the active workflow's layout.
- Presets Paradigm and Agents still exist (View > Layout > Preset) beside the Paradigm Editor and Agent Automation workflows; whether to drop or rename them is open.
- Built-in workflows only; no user folder, overlay, Manage Workflows content, crash-loop guard or missing-panel placeholder yet (Phase 2-3). No feature flag back to the enum: rollback = revert the commit (the file stays readable by the old build).
- Manage dialogs are empty placeholders (added 2026-10-19); no menu entry or shortcut opens them.
- No keyboard shortcut for sections yet.
