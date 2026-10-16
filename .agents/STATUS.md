# Status

Living tracker for Carbon. Update it at the end of each work session. Newest entries first in logs. Scope lives in `FEATURE.md`, decisions and open questions in `PROJECT.md`, evidence in `RESEARCH.md`.

**Last updated:** 2026-10-16

## Current focus

Research complete. Next: decide open questions, then start the Rust daemon scaffold.

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
| 1 Code editor | [ ] | |
| 2 Paradigm editor | [ ] | Needs research first |
| 3 Agents tab | [ ] | |
| 4 Multi-platform (Win/macOS/Linux) | [ ] | |
| 5 Lightweight | [ ] | Design focus, not first priority |
| 6 Modular extension sections | [ ] | |

### Infrastructure
- [x] Rust workspace scaffold (`apps/carbon-core` lib + `apps/carbon-gui` bin)
- [x] In-process core: `apps/carbon-core` (`project/built/carbon-core.md`)
- [ ] Daemon: workspace, buffers, terminals, LSP/DAP, agent sessions
- [~] UI shell: `apps/carbon-gui` custom title bar (C logo, File/Edit/View/Help menu, window controls) on gpui-kit `=0.7.3`; only Exit/Ctrl+Q works, other items placeholders; builds and opens on Windows, macOS/Linux untested
- [x] GPUI Kit knowledge base written (`project/ui/ui-framework.md`)
- [ ] CI for all three platforms

## Next steps
1. Answer open questions in `PROJECT.md` (native framework spikes, v1 scope, licence stance).
2. Run the paradigm editor research pass.
3. Define daemon crate layout and protocol (state-first, snapshot plus ordered actions).
4. Prototype GPUI Kit shell: window, theme tokens from `DESIGN.md`, dock layout, editor component; check unverified items in `project/ui/ui-framework.md` (LSP trait, editor latency, IME, screen readers, multi-window docking).

## Decision log
Format: date | decision | reason | where recorded.

| Date | Decision | Reason | Ref |
|---|---|---|---|
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

## Known issues and risks
| ID | Issue | Severity | Status | Workaround / plan |
|---|---|---|---|---|
| R-1 | Linux Vulkan/Wayland/NVIDIA issues for native GPU toolkits (webview risk closed by decision) | Medium | Open | Test early on Linux (spike S6) |
| R-2 | AGPL/ELv2 licences of some candidate components | Medium | Open | Licence review before bundling (`PROJECT.md`) |
| R-3 | Published latency/RAM comparisons conflict (2x-15x) | Low | Open | Run own benchmark |
| R-4 | GPUI Kit breaks API almost every minor (0.6, 0.7); 0.7.2 yanked; accessibility "Partial"; no terminal or diagram canvas in the kit | High | Open | Pin exact version; update `gpui-kit` only; wrap kit widgets behind Carbon components; test a11y early |

## Session log
Newest first. One short entry per session: what was done, what is next.

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
- Done: researched Rust GUI frameworks; wrote `documents/gui-framework.md` (12 spec tables, scoring, options A-D, spikes S1-S7). Raw notes in `research_notes/GUI framework comparison/` (kept until the document is accepted).
- Next: user decision on framework; run the spikes (editor latency, RAM, Linux webview, accessibility).
- Update (same day): user decided native desktop GUI, webview excluded. Reworked `gui-framework.md` for native only (lead: GPUI + gpui-component gated by editor and a11y spikes; fallback egui); updated `PROJECT.md`, `DESIGN.md`. Spikes now S1-S8 in the document.

### 2026-10-16 (later)
- Done: created `DESIGN.md` (Carbon UI rulebook, rules tagged by source); read all of `knowledge/` (read-only); added both to the docs map in `PROJECT.md`.
- Next: apply `knowledge/ARCHITECTURE.md` and `SECURITY.md` ideas to the daemon design (workspace-as-folder, jobs, write root, local-only server, keyring); decide UI approach.

### 2026-10-16
- Done: restructured `.agents/` into `PROJECT.md`, `RESEARCH.md`, `FEATURE.md`; added scoring (adoption 1-5, unique flag); renamed `PROJECTS.md` to `PROJECT.md`; added `STATUS.md`.
- Next: open questions in `PROJECT.md`; paradigm editor research.
