# Section Bar (archived 2026-10-22)

**Removed.** User decision 2026-10-22 (appearance rework): no sections and no Section Bar. Carbon has one layout; the command palette sits in the title bar (command center) and Extensions / Settings are under the status bar's app menu button (three lines). Current shell: `../built/title-bar-menu.md`. The text below is the doc as it was; the code it names was deleted (`section_bar.rs`, `SwitchSection`, View > Section, per-section `layout.json`).

---

Built 2026-10-16. User decision: a vertical bar like VS Code's activity bar, called the **Section Bar**; its icons switch the whole app to a different section, each with its own layout. Section contents are not built; placeholders only. Sections were called **workflows** (2026-10-19); the workflow system was dropped on 2026-10-22 (workflows are extensions; `../archived/workflow-system.md`). The code below is unchanged: the Manage Workflows button still opens its placeholder dialog, and its fate is open (`PROJECT.md`).

## What

| Part | Behaviour |
|---|---|
| Bar | Fixed 52 px strip left of the dock area, under the title bar; theme `title_bar` colours, right border; 8 px top padding |
| Icons | One ghost icon button per section, 36 px square (8 px clear each side), 20 px icon; active one shows selected; tooltip + accessibility label = section name |
| Switching | Click icon or View > Section (checkmark). Current section's layout is kept in memory, target's last layout loads (default the first time) |
| Bottom group | Pinned to the bar's bottom, top to bottom: Manage Extensions (Lucide `puzzle`), Manage Workflows (`workflow`), Manage Settings (`settings`). Same button style, never selected; tooltip = dialog title = that name; each opens its dialog (720 px wide, placeholder body: icon, "Nothing here yet."). No content yet. Planned: Extensions = official + user-added plugins, VS Code-like (compilers, linters, appearances); Workflows = add/remove workflows (dropped 2026-10-22: workflows are extensions); Settings = all user options (system, appearance, agents, ...) |
| Persistence | `target/carbon-gui/layout.json` = `{ active, sections: { <section>: DockAreaState } }`; restart reopens the active section with its layout |
| Reset Layout | Resets the active section to its default |

## Sections

| Section | Icon | Default layout (placeholders, `TestPanel`) | Status |
|---|---|---|---|
| Code Editor (`code`) | Lucide `code` ("< >") | Left: Explorer; center: Editor; right: Chat; bottom: Terminal | Explorer is a real panel (folder empty state, `workspace-folders.md`); others placeholders |

Adding a section: variant in `Section` (`section_bar.rs`) with label, icon, id; its default in `section_preset` (`workbench.rs`, maps to a `Preset` in `layouts.rs`); icon name in `icon_assets!` (`app_assets.rs`) if not in the kit default set.

## Code

| File | Role |
|---|---|
| `apps/carbon-gui/src/section_bar.rs` | `Section` enum, `SectionBar` view, dispatches `SwitchSection`; bottom Manage buttons |
| `apps/carbon-gui/src/manage.rs` | `Manage` enum (Extensions, Workflows, Settings), `open` = placeholder dialog |
| `apps/carbon-gui/src/workbench.rs` | `switch_section`, per-section layouts, `LayoutFile` save/restore, `reset_layout` |
| `apps/carbon-gui/src/app_assets.rs` | `AppAssets`: kit default icons + `icon_assets!(ExtraIcons, [Code])` |
| `apps/carbon-gui/src/menus.rs` | `SwitchSection` action, View > Section submenu |
| `apps/carbon-gui/src/layouts.rs` | `Preset::Code` = Code Editor default |
| `apps/carbon-gui/src/main.rs` | Row: Section Bar + workbench; keeps the bar's active icon in sync |

## Verified

- Builds clean; tests pass; app starts and writes `layout.json` with `active: code` and panels Explorer, Editor, Chat, Terminal.
- Not checked by hand: icon rendering, tooltip, selected style. Switching is untested in practice (only one section exists).

## Limits

- Old single-layout `layout.json` (T1/T2 format) is ignored on first start and replaced by the section default.
- Named layouts and presets are global, not per section; applying one replaces the active section's layout.
- Presets Paradigm and Agents still exist (View > Layout > Preset); they may become sections later.
- Manage dialogs are empty placeholders (added 2026-10-19); no menu entry or shortcut opens them.
- No keyboard shortcut for sections yet.
