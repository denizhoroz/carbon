# Not built yet

Committed features (decided by the user or in `PROJECT.md`) with no code yet. Details, evidence and scores stay in `FEATURE.md` (IDs below); this is only the to-build index. Proposed/Open rows are in `FEATURE.md` Part 1 and are not listed until committed. When a feature ships, move it to `../built/` (one file) and delete its row here. A feature that gets its own research or design doc before it is built puts that doc in this folder.

## Shell and UI

| Feature | Refs | Notes |
|---|---|---|
| Panel layout T3: pop-out OS windows, multi-monitor, top dock, guide diamond, float inside window | `../built/panel-layout.md` s7, `DESIGN.md` s4 | Deferred by user; kit has no cross-window docking |
| Command palette (every command, shortcut shown) | WB-01, `DESIGN.md` s4 | Menu item exists, no handler |
| Status bar rest: dock toggles, diagnostics, agents, cost, job spinner + Stop, clickable items (`../built/status-bar.md` has workspace/branch/jobs) | `DESIGN.md` s4 | |
| Recent folders list; reopen last folder on start; layout per folder | WB-03, WB-09 | Open/create/close built (`../built/workspace-folders.md`) |
| Explorer T3: multi-select, drag-drop move, copy/paste files, undo file ops, filter-on-type, compact folders, file nesting, git/problems/agent markers; whole-project background index (go-to-file, search) | WB-03 | T1 + T2 built (`../built/file-tree.md`) |
| Code Editor section contents: Editor (placeholder shows opened file), Chat, Terminal (placeholders), then problems/output | WB-03, WB-04, ED-01, TM-01, AG-01 | `test_panel.rs` placeholders; `../built/section-bar.md` |
| More sections in the Section Bar (paradigm, agents) and its bottom group (settings) | `DESIGN.md` s4 | Presets Paradigm/Agents exist as layouts only |
| Per-workspace layout (now one global file) | WB-09 | Needs workspace concept |
| Settings UI, keymap, VS Code settings/keymap/theme import | ST-01, ST-04, ST-06 | |
| Accessibility audit (Narrator, VoiceOver, Orca) | AX-01, `DESIGN.md` s3 | Kit a11y "partial" (R-4) |

## Pillar 1: Code editor

| Feature | Refs |
|---|---|
| Core editing: multi-cursor, find/replace, folding, format, auto save/hot exit, diff editor | ED-01..04 |
| Tabs/splits for editors, explorer, problems | WB-02, WB-03, WB-04 |
| Language intelligence: LSP, navigation/refactor, tree-sitter, semantic tokens | LS-01..04, LS-06 |
| Debugging: DAP, launch.json, logpoints | DB-01, DB-02 |
| Workspace search | SR-01 |
| Git: hunks, blame, history, merge editor, PR | SC-01, SC-03, SC-04, ED-06 |
| Integrated terminal + shell integration | TM-01, TM-02 |

## Pillar 2: Paradigm editor

| Feature | Refs |
|---|---|
| Whole pillar; research pass first (diagram tools, formats, AI integration, licences) | `FEATURE.md` Pillar 2, `PROJECT.md` open questions |

## Pillar 3: Agents tab

| Feature | Refs |
|---|---|
| Multiple sessions in panes/windows | AG-01, SE-01, SE-03 |
| External agents via ACP; MCP tools | AG-06, MS-01, SE-04 |
| Approvals, plan mode, policy engine, hooks | AP-01..04, AP-06 |
| OS sandbox default on; destructive-git guard | AP-11, AP-12 |
| Checkpoints incl. side effects | CK-01, CK-02 |
| AGENTS.md / CLAUDE.md / SKILL.md / .mcp.json | CX-01, MS-02 |
| Model picker, BYOK/local, live cost meter | MD-01, MD-09, MD-10 |

## Pillars 4-6 and core

| Feature | Refs |
|---|---|
| Daemon split of `carbon-core` (now in-process: `../built/carbon-core.md`); LSP/DAP, git, file watching in core | RM-06, `PROJECT.md` |
| Terminal emulation for the Terminal panel (VT parser + grid; must answer ConPTY `ESC[6n`) | TM-01, `../built/carbon-core.md` findings |
| Agent backend for sessions (ACP / direct APIs) on the core session model | AG-06, SE-04 |
| macOS and Linux builds + CI on all three | PL-12, `STATUS.md` |
| Startup/RAM budgets and benchmarks | PL-11 |
| WASM extensions with capability grants | EX-06 |
| MCP servers / skills / hooks as extension types | EX-02, MS-01, MS-02, MS-04 |
| Declarative VSIX tier, LSP/DAP launchers | EX-01, EX-03 |
| Open VSX + own gallery, signed, allow-list | EX-08, EX-09 |
