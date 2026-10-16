# Built features

One file per feature that exists in code. Each says what was built, where, how to use it, limits, and what was verified. When a feature ships, add its file here and remove its row from `../not_built/README.md`.

| Feature | File | Code | Built | Checked by hand |
|---|---|---|---|---|
| carbon-core: workspace, file tree, buffers, terminals (PTY), agent session model | `carbon-core.md` | `apps/carbon-core/` | 2026-10-16 | n/a (library; 27 tests pass) |
| Workspace folders: open / create / close (File menu, status bar button, Explorer empty state) + real Explorer panel shell | `workspace-folders.md` | `apps/carbon-gui/src/{workspace_ops,explorer_panel}.rs`, core `workspace.rs` | 2026-10-16 | Click-tested 2026-10-17 (Ctrl+K Ctrl+O untested) |
| Status bar (workspace, git branch, jobs) + `carbon-gui <folder>` | `status-bar.md` | `apps/carbon-gui/src/status_bar.rs`, core `vcs.rs`, `jobs.rs` | 2026-10-16 | Screenshot only |
| Custom title bar + menu bar | `title-bar-menu.md` | `apps/carbon-gui/src/{title_bar,menus}.rs` | 2026-10-16 | No (opens, no crash) |
| Theming and fixed styles | `theming.md` | `apps/carbon-gui/src/style/` | 2026-10-16 | No (themes load, test passes) |
| Section Bar (vertical section switcher; Code Editor section with Explorer/Editor/Chat/Terminal placeholders) | `section-bar.md` | `apps/carbon-gui/src/{section_bar,app_assets}.rs`, `workbench.rs` | 2026-10-16 | No (starts, layout file checked) |
| Panel layout T1 + T2 (docks, drag/snap, named layouts, presets, keyboard move/resize, auto-hide) | `panel-layout.md` (comparison + scope + implementation) | `apps/carbon-gui/src/{workbench,layouts,test_panel}.rs` | 2026-10-16 | No (T2 not yet run) |

Platforms: built and started on Windows only; macOS and Linux untested for every row.
