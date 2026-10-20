# Status

Living tracker for Carbon. Update it at the end of each work session. Newest entries first in logs. Scope lives in `FEATURE.md`, decisions and open questions in `PROJECT.md`, evidence in `RESEARCH.md`.

**Last updated:** 2026-10-20

## Current focus

Core panels work on Windows (shell, layout, Explorer, editor with rust-analyzer, terminal, command palette, chat GUI with a demo agent). Next: a real chat backend (ACP/API), Manage dialog contents (Extensions, Workflows, Settings), paradigm editor research, then macOS/Linux builds and CI.

## Progress

Legend: `[x]` done, `[~]` in progress, `[ ]` not started, `[!]` blocked.

### Research and planning
- [x] Landscape of agent workspaces and platforms (`RESEARCH.md` s2-3)
- [x] VS Code reference: features, architecture, forks (`RESEARCH.md` s4)
- [x] VS Code vs alternatives comparison and synthesis (`RESEARCH.md` s5-6)
- [x] Feature catalog with adoption scores (`FEATURE.md` Part 2)
- [x] Committed pillars defined (`PROJECT.md`)
- [ ] Paradigm editor research (diagram/architecture tools)
- [ ] Own benchmarks: native framework spikes, keystroke latency, RAM, large files
- [ ] Decide unassigned high-score features (see `FEATURE.md` 1.4)

### Pillars (implementation)
| Pillar | State | Notes |
|---|---|---|
| 1 Code editor | [~] | Tier 1 built: tabs, save, colours, rust-analyzer (`project/built/code-editor.md`); terminal tier 1 (`project/built/terminal.md`) |
| 2 Paradigm editor | [ ] | Needs research first |
| 3 Agents tab | [~] | Chat panel v1 GUI with demo agent (`project/built/chat.md`); no real backend, sessions or approvals yet |
| 4 Multi-platform (Win/macOS/Linux) | [ ] | |
| 5 Lightweight | [ ] | Design focus, not first priority |
| 6 Modular extension sections | [ ] | |

### Infrastructure
- [x] Rust workspace scaffold (`apps/carbon-core` lib + `apps/carbon-gui` bin)
- [x] In-process core: `apps/carbon-core` (`project/built/carbon-core.md`)
- [ ] Daemon: workspace, buffers, terminals, LSP/DAP, agent sessions
- [~] UI shell: `apps/carbon-gui` custom title bar (C logo, File/Edit/View/Help menu, window controls) on gpui-kit `=0.7.3`; File, Edit and View items work, command palette + Quick Open built (`project/built/command-palette.md`), About is a placeholder; builds and opens on Windows, macOS/Linux untested
- [x] GPUI Kit knowledge base written (`project/ui/ui-framework.md`)
- [ ] CI for all three platforms

## Next steps
0. Workflow system Phase 3 (`project/not_built/workflow-system.md` s5): Manage Workflows dialog (list with status/reasons, enable/disable, re-enable quarantined, hide, duplicate, reset, remove, open folder), per-workflow named layouts, Section Bar scroll arrows.
1. Real backend behind `ChatBackend` (ACP or an API); `@` mentions; chat list.
2. Language-tool install model and Manage Extensions contents (`documents/language-tools.md`).
3. Paradigm editor research pass (pillar 2).
4. Build and test on macOS and Linux; CI for all three platforms.
5. Answer remaining open questions in `PROJECT.md` (v1 scope, licence stance, CRDT); check unverified items in `project/ui/ui-framework.md` (IME, screen readers, multi-window docking).
6. Later: daemon split with a state-first protocol.

## Decision log
Format: date | decision | reason | where recorded.

| Date | Decision | Reason | Ref |
|---|---|---|---|
| 2026-10-20 | Workflow storage: shipped packages `apps/carbon-workflow/{editor,paradigm,agent}/` (read-only next to the binary; Editor not removable), downloads in the OS local data dir, preferences and workflow list in the OS config dir; all three locations user-changeable in installer/Settings (docs only until Settings exists) | user decision | `project/not_built/workflow-system.md` s4.7, Q15-Q16; `PROJECT.md` |
| 2026-10-20 | View > Layout presets Paradigm and Agents removed; the Paradigm Editor and Agent Automation workflows replace them | user answer (review point 5) | `project/built/workflows.md`, `project/not_built/workflow-system.md` s5.2 |
| 2026-10-20 | Workflow manifest format: TOML (JSON accepted for the same schema) | user answer Q1 | `project/not_built/workflow-system.md` s6 |
| 2026-10-20 | Built-in workflows change only via overlay or Duplicate; the embedded file is never edited | user answer Q2 | `project/not_built/workflow-system.md` s6 |
| 2026-10-20 | Q3 decided: recommended default. Patch overlay for tweaks of built-in workflows; whole-manifest copy for Duplicate | user answer Q3 | `project/not_built/workflow-system.md` s4.2, s6 |
| 2026-10-20 | Workflows span any domain (3D, sound, automation, diagrams); the contract is not tied to code, a text buffer or a workspace folder (`requires_workspace` optional; surface panel kinds, device capabilities, background policy added to the design; surface safety is open Q11) | user knowledge | `project/not_built/workflow-system.md` s3.1 |
| 2026-10-20 | Paradigm Editor and Agent Automation become workflows now as placeholders (empty panels, Section Bar icons) to test and design the workflow system (reverses the recommended default) | user answer Q4 | `project/not_built/workflow-system.md` s6 |
| 2026-10-20 | Section Bar overflow: up/down arrows to scroll the list plus mouse wheel; no fixed cap (replaces overflow menu) | user answer Q5 | `project/not_built/workflow-system.md` s4.10, s6 |
| 2026-10-20 | User workflow files in the OS config dir `Carbon/workflows/` | user answer Q6 | `project/not_built/workflow-system.md` s4.7, s6 |
| 2026-10-20 | "Code Editor" renamed to "Editor"; id `carbon.editor`, `legacy_ids = ["code"]`; UI label and docs renamed 2026-10-20 (code: section_bar.rs, layouts.rs; ids unchanged) | user answer Q7 | `project/not_built/workflow-system.md` s6, `project/not_built/README.md` |
| 2026-10-20 | Named layouts are per workflow; each workflow has its own layout config the user can change | user answers Q8, Q10 | `project/not_built/workflow-system.md` s4.7, s6 |
| 2026-10-20 | Scoped keybindings deferred: workflow system ships without scoped keys, no GPUI key-context spike for now | user answer Q9 | `project/not_built/workflow-system.md` s4.6, s6 |
| 2026-10-19 | No preinstalled language tools (servers, linters, formatters, compilers); users install them via Manage Extensions. Binary source model (config only / download PATH-first / bundled) open | user decision after research on VS Code, Zed, Neovim/Mason, Helix, JetBrains | `documents/language-tools.md`, `PROJECT.md` |
| 2026-10-19 | Section Bar bottom group: Manage Extensions, Manage Workflows, Manage Settings (dialogs, placeholders now). Workflows = sections (Code Editor; planned Paradigm Editor, Agent Automation; user custom workflows). Extensions = official + user-added plugins, VS Code-like (compilers, linters, appearances). Settings = all user options (system, appearance, agents, ...) | user decision | `PROJECT.md`, `DESIGN.md` s4, `project/built/section-bar.md` |
| 2026-10-19 | Chat panel v1 is GUI only: `ChatBackend` interface in the core + streaming demo agent (labelled); IDE layout; edit & retry, context chips, status label; chats saved per tab in `target/carbon-gui/chats/`; chat model separate from `AgentSession`; files kept when a tab closes | user decision (no model loader or API now) after comparing VS Code, Zed, Cursor, JetBrains | `project/built/chat.md` |
| 2026-10-19 | Every panel can move into every region, both ways (terminal/chat into the editor area, editor into docks); a region's last panel is draggable (kit patch 6); emptied center refills, emptied dock closes | user request after VS Code (terminal in editor area) and Zed (center terminal) | `project/built/panel-layout.md` s8 |
| 2026-10-19 | Command palette: Ctrl+Shift+P / F1 (Ctrl+K stays a chord prefix), Ctrl+P files, Ctrl+G line; one input with VS Code prefixes; `nucleo-matcher` fuzzy + usage tie-break; Zed look (under the title bar, no backdrop), drawn by `CarbonApp` not the kit Dialog; menus are the command source; Ctrl+P / F1 taken from the terminal's shell | user decision after comparing Superhuman, Linear, Raycast, VS Code, Retool, Warp, Zed | `project/built/command-palette.md` |
| 2026-10-18 | Terminal = `alacritty_terminal` emulator in the core + `portable-pty` + own GPUI view; tier 1 = base, selection/copy/paste/scrollback, profiles, shell integration (OSC 633/133), links, find; Windows default PowerShell 7, else Windows PowerShell | user decision after comparing VS Code, Zed, Lapce, WezTerm, Warp, Windows Terminal, Ghostty, JetBrains | `documents/terminal.md`, `project/built/terminal.md` |
| 2026-10-17 | Every Code Editor panel behaves like editor tabs: tab + close button alone or with others; Chat/Terminal addable many times (View > Add Panel, "..." > New); Explorer: one instance, plain "Explorer" title, not closable (user, same day). Supersedes "side panels keep their title" | user decision (changed mind) | `project/built/panel-layout.md` s8 |
| 2026-10-17 | Also vendor `gpui-base` 0.7.3 (`vendor/gpui-base`, patch: gutter gap); title/status bar 30 px; dialogs centered (kit patch 4) | user appearance requests; gutter gap and dialog position are hard-coded in the kit | `vendor/*/CARBON-PATCHES.md` |
| 2026-10-17 | Vendor gpui-component 0.7.3 with Carbon patches (`vendor/gpui-component`, `[patch.crates-io]`): editor tabs always shown (also with one file), 30 px tabs, panel-handled tab close | user decision (editor tab bar must not change style; kit had only a dock-wide setting and fixed 32 px tabs) | `vendor/gpui-component/CARBON-PATCHES.md`, `project/built/code-editor.md` |
| 2026-10-17 | Editor = gpui-kit `Editor` view + core buffer as source of truth (fork or own view later if the kit blocks); tier 1 = tabs/save, tree-sitter colours, speed gate, language servers | user decision after comparing VS Code, Zed, others | `documents/code-editor.md`, `project/built/code-editor.md` |
| 2026-10-17 | Code folding and soft wrap off in the editor | measured: kit folding delays each keystroke ~25 ms, 12.6 s to open a 200k-line file, stack overflow on a big broken parse; wrap: 2.4 s open | `project/built/code-editor.md` |
| 2026-10-17 | Language servers from a fixed table now; installable per-language packages later | user decision | `project/not_built/README.md` |
| 2026-10-17 | Git reads with gitoxide (`gix` 0.89, status only) for Explorer markers; problem/agent markers as API first; Explorer T3 extras: copy/cut/paste + undo, no file nesting | user decision | `project/built/file-tree.md`, FEATURE SC-06 |
| 2026-10-16 | In-process `carbon-core` crate before real panels (option a); daemon later | keep panel state out of views; cheap now | `project/built/carbon-core.md`, `PROJECT.md` |
| 2026-10-16 | Section Bar: left vertical bar switching sections, each with own layout; first section Code Editor | user decision | `project/built/section-bar.md`, `DESIGN.md` s4 |
| 2026-10-16 | Panel layout scope: T1 + T2 now, T3 (pop-out windows, top dock, guide diamond) later | user decision after comparison | `project/built/panel-layout.md` s7 |
| 2026-10-16 | Custom title bar with classic menu row (Win/Linux), native menu bar (macOS); placeholder "C" logo | user decision | `DESIGN.md` s4 |
| 2026-10-16 | Native desktop GUI, no webview | user decision | `PROJECT.md`, `documents/gui-framework.md` |
| 2026-10-16 | GUI framework: GPUI + gpui-component | user decision (chosen over Slint, egui, Xilem, others) | `PROJECT.md`, `documents/gui-framework.md` |
| 2026-10-16 | Six committed pillars (editor, paradigm editor, agents tab, multi-platform, lightweight, modular extensions) | Set by the user | `PROJECT.md` |
| 2026-10-16 | Do not fork VS Code; reuse open protocols and parts | Marketplace licensing, merge burden, closed Remote/Pylance | `RESEARCH.md` s4-5 |
| 2026-10-16 | Rust daemon with swappable UI client | Same binary local and remote, agent sessions survive UI | `RESEARCH.md` s5 |

## Error and solution log
Record each problem once: what happened, cause, fix, how to avoid. Newest first. Keep entries short.

Template:
```
### YYYY-MM-DD Short title
- Symptom: exact error text or behaviour
- Context: command, file, platform, version
- Cause: (verified | suspected)
- Fix: what worked
- Prevention: check or test added, if any
```

### 2026-10-17 Explorer buttons do nothing after a menu or dialog closes
- Symptom: clicking Open/Create Folder in the Explorer has no effect; no error
- Context: `window.dispatch_action(Box::new(CreateFolder))` from a panel button; handlers are `.on_action` on `CarbonApp`
- Cause (verified, gpui-pre `Window::dispatch_action`): it starts at the focused node; closing a popup menu left nothing focused, so it started at the window root (kit `Root`), above `CarbonApp`. Keyboard shortcuts fail the same way in that state
- Fix: `cx.on_focus_lost` in `CarbonApp::new` refocuses the app's focus handle (Zed workspace does the same)
- Prevention: comment in `main.rs`

### 2026-10-17 Create Folder dialog closes on an invalid name
- Symptom: notification "Cannot create folder: invalid folder name: a?b" shows, but the dialog closes
- Context: Enter in the Name input; kit 0.7.3 dialog
- Cause (verified, gpui-base `dialog.rs`): the dialog binds `enter` to `Confirm`; the single-line input propagates Enter; the default `on_ok` returns true and closes
- Fix: dialog `.on_ok(..)` runs create and returns success; removed the input `PressEnter` subscription; Create button closes only on success
- Prevention: kit note in `project/ui/ui-framework.md`

### 2026-10-16 gpui-kit `test-support` feature does not resolve
- Symptom: `failed to select a version for cocoa` (`gpui-fast-macos` wants `=0.26.0`, `gpui-pre-macos` locked 0.26.1); after `--precise 0.26.0` the same for `unicode-properties`
- Context: `[dev-dependencies] gpui-kit = { features = ["test-support"] }`, kit 0.7.3
- Cause (verified): feature list names `gpui-fast?/test-support`; resolution pulls the optional `gpui-fast` stack whose exact pins conflict with `gpui-pre`
- Fix: none; dropped the dev-dependency (no `TestAppContext` UI tests for now)
- Prevention: retry on next kit release; dock round-trip test was written and removed (see layout T1 session)

### 2026-10-16 `#[test]` fails: recursion limit reached
- Symptom: `error: recursion limit reached while expanding #[test]`
- Context: `mod tests { use super::*; }` in a file with `use gpui_kit::*;`
- Cause (verified): glob re-exports GPUI's `test` macro, shadowing std `#[test]`
- Fix: import named items in test modules, not `super::*`
- Prevention: comment in `apps/carbon-gui/src/style/theme.rs` tests

### 2026-10-17 Editor crash: stack overflow on a background thread
- Symptom: `thread '<unknown>' has overflowed its stack`, app exits
- Context: 200k-line Rust file, letters typed at the start (parse error), then switching tabs; release build
- Cause (verified by toggling): kit `extract_fold_ranges` (gpui-component `highlighter/input_adapter.rs`) recurses over the syntax tree; error recovery on a big broken file makes it deep
- Fix: editor folding off (`EditorState::folding(false)`)
- Prevention: comment in `editor_panel.rs`; folding stays off until fixed upstream or forked

### 2026-10-17 Editor: menu Save did nothing; read-only ignored
- Symptom: tab menu "Save" had no effect; std file opened read-only still accepted typing
- Cause (verified): closing the popup leaves focus empty, so the action starts at the window root, above the panel handler; the kit `Editor` element applies its own `readonly` prop each render, overriding `EditorState::set_readonly`
- Fix: Ctrl+S / Ctrl+W bound app-wide with root handlers acting on the active editor; pass `.readonly(..)` to the `Editor` element
- Prevention: notes in `project/built/code-editor.md`

## Known issues and risks
| ID | Issue | Severity | Status | Workaround / plan |
|---|---|---|---|---|
| R-1 | Linux Vulkan/Wayland/NVIDIA issues for native GPU toolkits (webview risk closed by decision) | Medium | Open | Test early on Linux (spike S6) |
| R-2 | AGPL/ELv2 licences of some candidate components | Medium | Open | Licence review before bundling (`PROJECT.md`) |
| R-3 | Published latency/RAM comparisons conflict (2x-15x) | Low | Open | Run own benchmark |
| R-4 | GPUI Kit breaks API almost every minor (0.6, 0.7); 0.7.2 yanked; accessibility "Partial"; no terminal or diagram canvas in the kit | High | Open | Pin exact version; update `gpui-kit` only; wrap kit widgets behind Carbon components; test a11y early. Since 2026-10-17 `gpui-component` and `gpui-base` are vendored with patches: re-apply `vendor/*/CARBON-PATCHES.md` on every kit upgrade |

## Session log
Newest first. One short entry per session: what was done, what is next.

### 2026-10-20 (workflow packages)
- Done: Carbon's own workflows moved to package folders `apps/carbon-workflow/{editor,paradigm,agent}/` (loaded from `carbon-workflow/` next to the binary or the repo; Editor compiled in as fallback with a notification); installed packages load from `%LOCALAPPDATA%\Carbon\workflows\installed\<id>\<version>\` (newest semver); env overrides for all three locations; `removable` / `can_turn_off` rules, Editor never blocked. 150 tests, clippy, fmt green; click-tested (repo packages, empty shipped folder).
- Next: release packaging must copy `apps/carbon-workflow/`; Phase 3.

### 2026-10-20 (workflow system Phase 2)
- Done: user workflows from `<config>/Carbon/workflows/user/`, overlays for built-ins, `state/state.json` (migrated once from `layout.json`, which is no longer written), fingerprint reset of stale layouts, unknown panel kind as placeholder, `[[commands]]`/`[[menus]]`, crash-loop guard with quarantine and automatic safe mode, BOM-tolerant parsing, presets Paradigm/Agents removed; `dirs` 6.0.0 added (already in the lock). 146 workspace tests, clippy, fmt green; click-tested with `CARBON_WORKFLOWS_DIR` (state files compared, two empty test chats removed). Doc: `project/built/workflows.md`.
- Next: Phase 3.

### 2026-10-20 (workflow system Phase 0-1)
- Done: `carbon-core::workflow` (`WorkflowId`, TOML manifests, validation, `Registry` with three embedded built-ins, `LayoutFile` kept readable by older builds); GUI: Section Bar, View > Section and workbench read the registry (`Section` enum removed), `layouts::PanelKind` + `LayoutSpec`, `--safe-mode`; `toml` 1.1.8 and `semver` 1.0.28 added (both already in the lock). 14 core + 3 GUI tests; workspace tests, clippy, fmt green; click-tested in a separate target dir (state files restored, one empty test chat file removed).
- Next: Phase 2.

### 2026-10-20 (workflow decisions + Editor rename)
- Done: Q3 decided (patch overlay; whole copy for Duplicate); workflow range (diagram, automation, 3D view, sound design, user-built) folded into `project/not_built/workflow-system.md`, `PROJECT.md`, `FEATURE.md`; "Code Editor" renamed to "Editor" in the UI label (`section_bar.rs:27`) and docs (ids and `layout.json` key unchanged). Not built or run after the rename.
- Next: workflow system Phase 0-1.

### 2026-10-20 (workflow system design)
- Done: design doc `project/not_built/workflow-system.md` (alternatives survey, manifest, layers, registry/lifecycle, panel kinds, safe mode, 6-phase migration from the `Section` enum); `project/not_built/README.md` rows. No code changed.
- Decisions (user answers, same day): Q1-Q2, Q4-Q10 decided (see Decision log); Q3 decided as the recommended default (patch overlay; whole manifest for Duplicate). New user knowledge: workflows range over diagram editing, automation, 3D display and sound design, so the design gained s3.1 (range, requirements W1-W7) and open Q11-Q14. Doc status: design accepted.
- Next: start Phase 0-1 (id adapter, then registry with three built-ins).

### 2026-10-20 (docs refresh)
- Done: read the project state; fixed stale docs (`PROJECT.md` status, sequencing, open question and docs map; `STATUS.md` focus, next steps, Agents pillar row; `README.md` git note and file table); removed `research_notes/` (staged `git rm`, user commits).
- Next: see Next steps. `FEATURE.md` is 88 KB and could be compacted.

### 2026-10-19 (chat panel)
- Done: core `chat` module (model, backend interface, demo agent, store) + 15 tests; GUI `chat_panel` (transcript, composer, chips, stop/retry/edit/clear, persistence, palette commands); placeholders and presets migrate. Click-tested in a test build (fixed: user-message action row gap, edit caret). 116 tests pass + 4 ignored; clippy and fmt clean.
- Next: a real backend (ACP or an API) behind `ChatBackend`; `@` mentions; chat list.

### 2026-10-19 (themes)
- Done: terminal background = code editor background; `gruvbox.json` with six variants (UI, syntax, terminal ANSI); Gruvbox Dark Hard is the default dark theme; theme test now rejects unknown keys (caught 9 stale kit names). Screenshot-checked dark hard.
- Next: theme picker in the palette (live switch, no restart).

### 2026-10-19 (drop zones)
- Done: drop-as-tab zone made the big one: split strips a quarter of the side, max 80 px (gpui-base patch 3); click-tested merge vs. left split; your layout restored after the test.

### 2026-10-19 (panels across regions)
- Done: found the block (kit refuses to drag a region's last panel); gpui-component patch 6; `Workbench::repair_layout` (center refill, empty docks close) on every layout change; `MoveBesideEditor` (View > Panel > Move Next to Editor, palette). Click-tested drags both ways, refill, keyboard move, restart. 96 tests pass + 4 ignored; clippy and fmt clean.
- Next: drop targets on closed docks / window edges (VS Code-style); commit (user).

### 2026-10-19 (command palette)
- Done: research (Superhuman, Linear, Raycast, VS Code, Retool, Warp, Zed); core `fuzzy.rs` (nucleo-matcher) + `file_index.rs` + `files_generation`; gui `commands.rs` registry from the menus + extras, `palette.rs` (commands `>`, files, line `:`, help `?`), usage ranking file, full-chord shortcut chips, focus-restoring dispatch; View > Go to File / Go to Line; palette keys kept from the terminal. Click test found `available_actions` dropping actions with fields (fixed: per-action `is_action_available`). 96 tests pass + 4 ignored; clippy and fmt clean; click-tested with F1 (Ctrl shortcuts untested).
- Next: try Ctrl+Shift+P / Ctrl+P / Ctrl+G and click-outside by hand; `@`/`#` once LSP symbols exist; commit (user).

### 2026-10-18 (terminal tier 1)
- Done: comparison (`documents/terminal.md`); core emulator (`terminal/{mod,emulator,osc,shells}.rs`, Carbon shell integration scripts for PowerShell/bash/zsh); GUI `terminal_panel` (canvas drawing, keystroke interceptor, text input handler, selection/copy/paste, scrollback, find, links, exit-code marks, profiles in "...", restart, restore). Fixed during testing: marks after ConPTY resize, "..." actions not reaching the panel, Git Bash MSYS cwd. Speed gate: 50 MB in 29 s (ConPTY limit 25 s), frame p99 1.6 ms, key echo median 0.3 ms. 67 tests pass + 4 ignored; click-tested.
- Next: language packages; agents using terminal `commands()`; commit incl. `vendor/` (user).

### 2026-10-18 (drag docks shut/open)
- Done: VS Code-style snap: drag a dock edge to the window edge to hide it, drag the edge back out to show it (left, right, bottom); closed bottom dock fully hidden (was a 29 px strip). Vendored kit patches: gpui-base 2, gpui-component 5. Click-tested all three docks both ways with simulated drags; 58 tests pass + 3 ignored.
- Then: dock collapse buttons removed from all tab bars (skin setting, no patch).
- Next: language packages; commit incl. `vendor/` (user).

### 2026-10-17 (appearance + panels)
- Done: title and status bar 30 px (= tab bar); Explorer name box full row height, plain border, no placeholder; dialogs centered (kit patch 4); 24 px gutter gap (vendored `gpui-base`, patch 1); every Code Editor panel has tabs + close, Chat/Terminal addable (View > Add Panel, "..." > New), Explorer closable and re-addable, emptied docks close. Fixed: the kit's "..." > Close closed unsaved editors without asking. 58 tests pass + 3 ignored; click-tested.
- Then (same day): Explorer back to its plain title, not closable; empty editor has no bar ("No file open").
- Next: language packages; commit incl. `vendor/` (user).

### 2026-10-17 (save and quit)
- Done: quit asks about unsaved files (Exit, Ctrl+Q, window x, Alt+F4; Save All / Don't Save / Cancel); File > New File (Ctrl+N, untitled tabs) and File > Save wired, untitled files ask a path; Edit menu uses the kit's text actions (Explorer maps them to file copy/paste/undo); tab colours and language server follow Save As and renames. Fixed: menu focus restore stole focus from a newly opened tab. 58 tests pass + 3 ignored; click-tested.
- Next: language packages; user's choice on LSP writes off the UI thread and the layout config folder; commit (user).

### 2026-10-17 (editor tabs)
- Done: vendored gpui-component 0.7.3 with 3 patches (always-tab panels, 30 px tabs, `request_close`); editor tabs keep the tab style with one file, x on every tab (unsaved asks), last tab clears to the empty editor, no extra content padding with several tabs. Click-tested; 57 tests pass + 3 ignored.
- Next: as below (language packages, commit).

### 2026-10-17 (code editor tier 1)
- Done: comparison of editor engines (`documents/code-editor.md`); core buffer sync (`set_rope`), disk reload/conflicts, LSP client (`lsp.rs`: stdio transport, store, UTF-16 positions); GUI editor tabs on the kit `Editor` (save, close dialog, restore, conflict bar, read-only outside folder), tree-sitter colours (13 grammars), rust-analyzer diagnostics/hover/completion/F12 + status bar item. Speed gate passed after turning folding and soft wrap off (big.rs 200k lines: open 52 ms, keys 2.5-9 ms). rust-analyzer installed with permission. 57 tests pass + 3 ignored. Click-tested.
- Next: installable language packages (user); commit; check Ctrl+S/Ctrl+W keys by hand; folding fix/fork; per-folder layouts.

### 2026-10-17 (file tree T3)
- Done: core `git_status` (gitoxide), `marks`, `history` + `move_entries` / `copy_entries` / `undo_file_op`, compact folders in `FileTree`, watcher flags git-relevant changes; GUI split into `explorer_panel/{rows,filter,file_ops}.rs`: multi-select, drag and drop (rows + OS files), copy/cut/paste, undo, type to filter, markers; background git status (max 1/s). 48 tests pass + 2 ignored (Recycle Bin, run once). Click-tested except multi-select and modifier keys; fixed list-wide drop highlight.
- Next: check multi-select / Ctrl shortcuts / OS drops / mouse wheel by hand; commit T1-T3; config folder for layouts; then the editor (real Editor panel) or Source Control.

### 2026-10-17 (file tree T2)
- Done: core `file_ops` (create file/folder, rename with buffers following, move to Recycle Bin via `trash` 5.2.9 MIT), `Core` methods refuse the root and outside paths; Explorer inline name box (new/rename), delete dialog, copy path / relative path, reveal in File Explorer, right-click menu, header New File/Folder buttons, F2/Delete/Shift+Alt+C/R keys; Editor shows "(deleted)". 41 tests pass + 1 ignored (real Recycle Bin, run once). Click-tested; fixed missing header icons, long exists message, "1 lines".
- Next: config folder for layouts (`%APPDATA%\Carbon`); recent folders; Explorer T3 or the editor; check modifier shortcuts and mouse wheel by hand.

### 2026-10-17 (file tree T1)
- Done: compared tree designs (kit Tree vs own rows, lazy vs scan, watcher vs poll); user picked own rows + lazy + watcher, T1. Core `FileTree` + `FsWatcher` (`notify`), GUI Explorer rows (`uniform_list`), keyboard, collapse all, Editor placeholder panel shows the opened file. 37 tests pass; click-tested; fixed ignored-folder children not dimmed.
- Next: Explorer T2 (new / rename / delete / copy path / reveal); config folder for layouts (`%APPDATA%\Carbon`); check mouse-wheel scroll by hand.

### 2026-10-17 (format + click test)
- Done: `cargo fmt` on the whole workspace (formatting only). Click-tested workspace folders by window messages: all entry points pass after 3 fixes (dialog closed on error; Explorer buttons dead after a menu closed; Name not focused from Explorer). Paths now ellipsize at the start. Clippy clean, 30 tests pass.
- Next: kit menu shows only the first key of chords (Ctrl+K ...): decide single-key shortcut or accept; Ctrl+K Ctrl+O not tested (needs real keyboard); recent folders + reopen last.

### 2026-10-16 (workspace folders)
- Done: open / create / close folder from File menu, status bar button (upward menu) and Explorer empty state; Create Folder dialog with Browse; core `create_folder`, `create_workspace`, `close_workspace` + 4 tests (30 total pass); Explorer is the first real panel; layout version 2. Screenshots of both Explorer states.
- Next: click-test the new UI; recent folders + reopen last; Explorer file tree on `Core::list_dir`.

### 2026-10-16 (status bar)
- Done: kit `StatusBar` with workspace / branch / jobs; GUI now owns `Entity<Core>`; `carbon-gui <folder>` opens a workspace; core `vcs.rs` + `jobs.rs` + 5 tests (workspace 26 tests pass); fixed 2 clippy warnings in `workbench.rs`. Checked by screenshot (preview build in `target/preview` because the user's app held the exe).

### 2026-10-16 (carbon-core)
- Done: `apps/carbon-core` (workspace, file_tree, buffer, terminal, agent, error) + 18 integration tests, workspace member; whole workspace builds, 21 tests pass. Finding: ConPTY needs `ESC[6n` answered.
- Next (urgent list): commit to git; workspace + OS config dirs; panel conventions; editor/terminal checks; then Explorer panel on `Core::list_dir`.

### 2026-10-16 (spacing fix)
- Done: panel bodies padded 16 px (`PANEL_PADDING`, DESIGN s4) and text centred; Section Bar 52 px, 36 px button, 20 px icon, 8 px padding. Checked by screenshot (PrintWindow of the app window only; `Start-Process -NoNewWindow` so the debug console does not cover it).

### 2026-10-16 (Section Bar)
- Done: `section_bar.rs` (Section enum, bar view), `app_assets.rs` (extra Lucide `code` icon), per-section layouts in `workbench.rs` (`layout.json` now `{active, sections}`), View > Section, Code preset = Explorer/Editor/Chat/Terminal. Build + 3 tests pass; app starts; layout file verified.
- Next: check bar by hand; build section contents (explorer, editor, chat, terminal).

### 2026-10-16 (docs: built / not built)
- Done: `project/built/` (README index, `title-bar-menu.md`, `theming.md`, `panel-layout.md` moved from `documents/` + new s8 implementation); `project/not_built/README.md` (committed features without code, by pillar); refs updated; README rule added.

### 2026-10-16 (layout T2)
- Done: `layouts.rs` (presets Code/Paradigm/Agents; `LayoutStore` in `target/carbon-gui/layouts.json`: named layouts max 9 + auto-hide flags), `workbench.rs` T2 (save-as dialog, apply Ctrl+Alt+1..9, delete, presets, Reset = Code preset, move focused panel Ctrl+K arrows, resize focused dock Ctrl+Alt+Shift+arrows 24 px, auto-hide docks close on click outside and start hidden), menus rebuilt on change (View > Auto-Hide with checkmarks, View > Layout, View > Panel). `cargo check` clean, 3 tests pass. Not run: exe was locked by a running instance (PID 2808); nothing checked by hand.
- Next: manual T2 check (list in chat); keyboard move/resize need a focused panel (click inside it first).

### 2026-10-16 (layout T1)
- Done: `apps/carbon-gui/src/workbench.rs` (kit `DockArea` + `DockSkin`, close buttons on, default test layout, save on `LayoutChanged` debounced 500 ms to `target/carbon-gui/layout.json`, restore on start, fallback to default), `test_panel.rs` (one restorable `TestPanel` type, title in `PanelInfo::Panel`), View menu: Toggle Left/Right/Bottom Dock (Ctrl+B / Ctrl+Alt+B / Ctrl+J), Reset Layout. Builds clean; app starts. Drag/snap/resize/close/zoom/restore not yet checked by hand.
- Next: manual T1 check; then T2 (named layouts, task presets, keyboard move/resize, auto-hide). Reset Layout pulled forward from T2 so closed panels can return.

### 2026-10-16 (panel layout research)
- Done: `project/built/panel-layout.md` (11 products incl. GPUI Kit dock, 4 feature tables, verdict, tiers); FEATURE WB-09; README/PROJECT/DESIGN refs.
- Next: build T1 shell with kit `DockArea` (left files, center editor, right agents, bottom terminal placeholders), then T2.

### 2026-10-16 (styling)
- Done: `apps/carbon-gui/src/style/`: `fixed.rs` (non-themeable consts: window size, logo, title bar gap), `theme.rs` (kit `ThemeRegistry::watch_dir` on `style/themes/`, hot reload; `style/settings.json` picks `mode` dark|light|system + `dark_theme`/`light_theme`), `themes/carbon.json` (Carbon Dark/Light). Test `theme_files_parse` checks every theme file + settings names. Window 1280x800.
- Next: move themes/settings to OS user config dir; `system` mode does not follow live OS changes yet; settings.json read at startup only (theme files hot-reload); contrast check per DESIGN s3 not automated.

### 2026-10-16 (title bar + menu)
- Done: `apps/carbon-gui/src/{main,menus,title_bar}.rs`: `TitleBar` + `AppMenuBar`, placeholder actions, `Quit` on `secondary-q`; `DESIGN.md` s4 menu rule changed.
- Next: check by hand on Windows (menus open, arrows/Esc, drag, double-click maximise, window buttons); macOS/Linux.

### 2026-10-16 (GPUI test window)
- Done: root `Cargo.toml` -> virtual workspace (resolver 3, edition 2024, `gpui-kit = "=0.7.3"` in workspace deps, dev opt-level list); `apps/carbon-gui` hello window with click counter (section 2 skeleton works on 0.7.3). First dev build 2m31s on Windows (rustc 1.98.1, MSVC, CMake 4.4.2).
- Next: add `carbon-core` to workspace; quit action/keybinding (kit installs none); try on macOS/Linux.

### 2026-10-16 (GPUI Kit)
- Done: GPUI Kit chosen by the user; read gpui-kit.com docs (llms-full dump, ~60 pages); wrote `project/ui/ui-framework.md`; corrected version facts in `PROJECT.md` and `documents/gui-framework.md` (kit depends on crates.io `gpui-pre` `=0.3.9`, no git rev needed; Rust 1.92+); added risk R-4.
- Next: prototype shell; verify the (?) items in the knowledge base.

### 2026-10-16 (GUI framework)
- Done: researched Rust GUI frameworks; wrote `documents/gui-framework.md` (12 spec tables, scoring, options A-D, spikes S1-S7). Raw notes in `research_notes/GUI framework comparison/` (deleted 2026-10-20 after the user chose GPUI; recoverable from git history).
- Next: user decision on framework; run the spikes (editor latency, RAM, Linux webview, accessibility).
- Update (same day): user decided native desktop GUI, webview excluded. Reworked `gui-framework.md` for native only (lead: GPUI + gpui-component gated by editor and a11y spikes; fallback egui); updated `PROJECT.md`, `DESIGN.md`. Spikes now S1-S8 in the document.

### 2026-10-16 (later)
- Done: created `DESIGN.md` (Carbon UI rulebook, rules tagged by source); read all of `knowledge/` (read-only); added both to the docs map in `PROJECT.md`.
- Next: apply `knowledge/ARCHITECTURE.md` and `SECURITY.md` ideas to the daemon design (workspace-as-folder, jobs, write root, local-only server, keyring); decide UI approach.

### 2026-10-16
- Done: restructured `.agents/` into `PROJECT.md`, `RESEARCH.md`, `FEATURE.md`; added scoring (adoption 1-5, unique flag); renamed `PROJECTS.md` to `PROJECT.md`; added `STATUS.md`.
- Next: open questions in `PROJECT.md`; paradigm editor research.
