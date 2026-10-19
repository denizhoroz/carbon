# Panel layout comparison (move, snap, resize, detach, save)

Question (user, 2026-10-16): VS Code lets you close, move, snap and resize panels; is it the only or the best one? Data as of 2026-10-16.
Legend: **Y** yes, **P** partial, **N** no, **build** = Carbon must build it, `=` same as row above. Evidence per row: **doc** vendor docs/source, **A** forum/anecdotal, **inf** inferred, **(?)** unverified cell.

## 1. Products compared

| Product | Version / kind | Layout model in one line | Ev |
|---|---|---|---|
| VS Code | 1.142 | Fixed regions (2 side bars, panel, editor grid), views drag between regions; floating editor windows | doc |
| Cursor | VS Code fork | VS Code model + saved layout presets (agent/editor) | A |
| Visual Studio | 2022 | Free docking with guide diamond, float anywhere, auto-hide, named layouts | doc |
| JetBrains IDEs | 2026.x | Tool windows on edge slots, five view modes, separate frames, layouts | doc |
| Eclipse | 4.x | Drag views by tab, detach, trim stacks, perspectives | doc |
| Zed | 2026 | Three docks (L/R/B), panels pick a dock in settings, split panes, zoom | doc |
| Sublime Text | 4 | Layout presets (columns/rows/grid), groups, no dockable panels | doc, A |
| Neovim | 0.11+ | Keyboard splits, move to edge, floating windows API | doc |
| Blender | 4.5 / 5.0 | Areas split/join/swap by corner drag, workspaces (non-IDE reference) | doc |
| Dear ImGui | docking branch | Library: any window docks into any node, multi-viewport (reference) | doc, A |
| **GPUI Kit 0.7.3 dock** | Carbon's base | `DockArea`: center + L/R/B docks, Split/Tabs tree, drag/drop, dump/load | doc (source) |

## 2. Move and snap

| Product | Drag panel to edges | Top position | Drop-zone guides | Drop into tab group | Split editor by drag | Panel into editor area | Keyboard move |
|---|---|---|---|---|---|---|---|
| VS Code | Y: 2 side bars + panel (L/R/B/T) | Y (panel) | P: highlight | Y | Y: 4 dirs + grid presets | P: terminal, some views | Y: "View: Move View" |
| Cursor | = | = | = | = | = | = | = |
| Visual Studio | Y: any edge incl. IDE outer edges | Y | **Y: guide diamond**; Ctrl = no snap | Y | Y | Y: dock as tabbed document (inf) | P |
| JetBrains | Y: icon/header drag; "Move to" menu | P (?) | P | Y | Y (?) | N | Y: Move to, resize actions |
| Eclipse | Y: drop cursors, drag whole stack | Y | P: cursors | Y: stacks | Y | N: editors stay in editor area | P |
| Zed | P: 3 docks; some panels L/R only; right-click icon, written to settings | N | N | P: one visible panel per dock | Y: panes (?) | N | P |
| Sublime | N: side bar + bottom panel | N | N | N | P: presets; Origami plugin | N | Y: groups |
| Neovim | Y: `Ctrl-w H/J/K/L` to far edge | Y | N | N | Y: any split | n/a | **Y: keyboard-native** |
| Blender | Y: split, join, swap, replace areas | Y | P: join highlight | N: one editor per area | Y | n/a | P |
| Dear ImGui | Y: any window to any node | Y | Y: drop targets | Y | Y | Y | N |
| **GPUI Kit** | Y: center + L/R/B docks; drag between groups and regions | **N** | Y: drop indicator | Y | Y: edge drop splits (`split_placement_at`) | Y: one tree | build |

## 3. Detach and windows

| Product | Float inside window | Separate OS window | Multi-monitor | Always on top | Re-dock |
|---|---|---|---|---|---|
| VS Code | N | Y: editors, terminal, some views; restored on restart | Y | Y | Y: drag back |
| Cursor | = | = | = | = | = |
| Visual Studio | Y | Y: any tool or document window | Y | P (?) | Y: Ctrl + double-click title |
| JetBrains | **Y: Undock (overlay), Float** | **Y: "Window" mode; frames hold tool windows** | Y | P | Y |
| Eclipse | N | Y: detached views, grouped in one window | Y | N | Y: drag tab back |
| Zed | N: zoom is the workaround (A) | N: new windows only per project | P | N | n/a |
| Sublime | N | Y: drag tab out (default) | Y | N | Y |
| Neovim | Y: floating windows (`nvim_open_win`) | N: terminal-bound | N | n/a | Y |
| Blender | N | Y: area to new window (inf) | Y | N | P |
| Dear ImGui | Y | Y: multi-viewport (backend-dependent) | Y | P | Y |
| **GPUI Kit** | N | N: one `Root` per window; cross-window drag undocumented (?) | build | build | build |

## 4. Size and visibility

| Product | Drag resize | Keyboard resize | Maximize / zoom | Auto-hide (unpinned) | Close + restore | Reset layout |
|---|---|---|---|---|---|---|
| VS Code | Y | P | Y: panel (center alignment only), editor group | N: toggle only | Y | Y: "Reset View Locations" |
| Visual Studio | Y | P | Y | **Y: pin / auto-hide** | Y | Y |
| JetBrains | Y | **Y: Ctrl+Alt+Shift+arrows** | Y: Ctrl+Shift+' | **Y: Dock unpinned** | Y | Y: Shift+F12 |
| Eclipse | Y | P | Y | Y: minimize to trim stack | Y | Y: Reset Perspective |
| Zed | Y; `resize_all_panels_in_dock` | P | Y: zoom | N | Y | P |
| Sublime | Y | N | N | N | Y | P: Layout menu |
| Neovim | Y | Y: `Ctrl-w + - < > =` | P | N | Y | P |
| Blender | Y | P | Y: Ctrl+Space | N | Y | Y: reload workspace |
| **GPUI Kit** | Y | build | Y: zoom (`toggle_zoom`) | P: collapsible docks | Y | build (fallback layout) |

## 5. Persistence and named layouts

| Product | Persists per workspace | Named layouts | Quick switch | Task layouts |
|---|---|---|---|---|
| VS Code | Y | **N**: issue #95182 open since 2020 (backlog); profiles are not layouts | N | N |
| Cursor | Y; layout reset regressions (A) | Y: presets via gear menu (A) | Y: "View: Switch Layout" (A) | P: agent / editor presets |
| Visual Studio | Y | **Y: up to 10, rename, reorder** | Y: Ctrl+Alt+1..0 | P: one layout covers edit and debug |
| JetBrains | Y | Y: Window \| Layouts (locations + sizes) | P | N |
| Eclipse | Y | **Y: perspectives** (Save Perspective As) | Y | **Y: per-task perspectives** |
| Zed | Y (inf) | N | N | N |
| Sublime | Y | N (plugins) | P: layout presets | N |
| Neovim | P: `:mksession` | P: sessions, plugins | P | N |
| Blender | Y: in the `.blend` file | Y: workspaces | Y: workspace tabs | **Y** |
| Dear ImGui | Y: ini file | P: `DockBuilder` code | n/a | n/a |
| **GPUI Kit** | Y: `dump` / `load` (`DockAreaState`, serde) | build on dump/load | build | build |

## 6. Verdict

- **No single best.** VS Code is good but not the leader: no named layouts, no auto-hide, no float inside the window.
- **Visual Studio:** best snapping (guide diamond) and named layouts on hotkeys.
- **JetBrains:** most view modes (pinned, unpinned, undock, float, window) and keyboard resize.
- **Eclipse, Blender:** best task layouts (perspectives, workspaces); Blender split/join is the most free-form.
- **Zed:** most limited, and it runs on GPUI like Carbon. GPUI gives docks, not floating or cross-window docking, for free.
- **Pain points:** VS Code users ask for perspectives since 2020; Cursor layout resets (A); Dear ImGui split sizes differ per run and docking flags get lost (issues #6217, #3834).
- **Carbon opportunity:** Visual Studio snapping + JetBrains view modes + Eclipse/Blender task layouts (code, paradigm, agents) is a combination no single product ships.

## 7. Carbon scope (user decision 2026-10-16: T1 + T2 now, T3 later)

| Tier | Features | Effort on GPUI Kit | Status |
|---|---|---|---|
| T1 | Drag panels between L/R/B docks and center, drop indicator, split by edge drop, tab groups, resize, close, zoom, lock, persist per workspace | Kit `DockArea` as-is | Built 2026-10-16 (one global layout until workspaces exist) |
| T2 | Named layouts on hotkeys (Visual Studio); task presets: code, paradigm, agents (Eclipse/Blender); reset layout; keyboard move/resize (JetBrains); auto-hide docks | Own code on `dump`/`load`, `set_open`, actions | Built 2026-10-16 (`apps/carbon-gui/src/{workbench,layouts}.rs`), not yet checked by hand |
| T3 | Pop-out panels to OS windows, multi-monitor (DESIGN s4 multi-window); top dock; guide diamond; float inside window | Own build; kit has no cross-window docking or top dock | Deferred |

## 8. Carbon implementation (T1 + T2, built 2026-10-16)

| Feature | How | Shortcut / menu |
|---|---|---|
| Drag tabs between groups, docks, center; split on edge drop; resize; close; zoom | Kit `DockArea` + `DockSkin` (close buttons on) | Mouse |
| Toggle docks | `area.toggle_dock`; no collapse buttons in tab bars (`set_toggle_button_visible(false)`, user 2026-10-18) | Ctrl+B left, Ctrl+Alt+B right, Ctrl+J bottom; View menu |
| Persist current layout | Per section (`section-bar.md`): `dump` on `LayoutChanged`, 500 ms debounce, `load` on start, fallback section default | `target/carbon-gui/layout.json` |
| Task presets | `Preset::{Code, Paradigm, Agents}` (`layouts.rs`) | View > Layout > Preset |
| Reset layout | Active section's default (Code Editor = Code preset), reopens docks | View > Layout > Reset Layout |
| Named layouts (max 9) | `LayoutStore` (`target/carbon-gui/layouts.json`): `DockAreaState` + auto-hide flags | View > Layout > Save Layout As... (dialog), Apply (Ctrl+Alt+1..9), Delete |
| Keyboard move focused panel | `area.move_panel` into first tab group of target | Ctrl+K then left/right/down (docks), up (center); View > Panel |
| Keyboard resize focused dock | `set_dock_size` +-24 px, min 120 px | Ctrl+Alt+Shift+arrows |
| Auto-hide docks | Start hidden; open with toggle and take focus; close on click outside (bounds + dock size) | View > Auto-Hide Left/Right/Bottom Dock (checkmark) |
| Panels like editor tabs (2026-10-17, user) | Every Code Editor panel (Explorer, Chat, Terminal, placeholders) draws as a tab with a close button, alone or with others (`always_tab_bar`); x and "..." > Close work on a dock's last panel too (`request_close` -> `workbench::close_panel`); an emptied dock closes (also on load); closing the center's last panel leaves an empty editor | Tab x, "..." > Close |
| Drag a dock shut / open (2026-10-18, user; VS Code sash snap) | Drag a dock's edge toward the window edge: under 50 px (half the kit minimum, 100 px) it snaps closed and is fully hidden (bottom too, no strip); back over 50 px in the same drag it reopens at 100 px and follows the pointer. A closed dock with panels keeps a 5 px grab strip at its edge (left: next to the Section Bar; right: window edge; bottom: above the status bar): drag it out to reopen. Empty docks get no strip. Vendored kit: gpui-base patch 2, gpui-component patch 5 | Mouse; toggles unchanged |
| Explorer differs (2026-10-17, user) | The main panel: alone in its dock it shows the plain "Explorer" title (no tab, no close button; `always_tab_bar` and `closable` false); in a group with other panels it is a tab without x | View > Add Panel > Explorer if moved away |
| Big "drop as tab" zone (2026-10-19, user) | Dropping on a group: the edge strips (a quarter of the side, at most 80 px) split it left/right/top/bottom; everything else joins the group as a tab (was 35% per edge, leaving a 30% x 30% middle). gpui-base patch 3. Click-tested: 186 px in from the editor area's edge previews the whole group and merges; 31 px in previews the left half and splits | Drag a tab |
| Any panel into any region (2026-10-19, user) | Drag a tab into another region, also a region's last panel (was refused by the kit: gpui-component patch 6). Terminal or Chat beside, below, above the code in the editor area; the editor into a dock beside the terminals. An emptied editor area refills with an empty editor, an emptied dock closes (`Workbench::repair_layout` on every `LayoutChanged`). Keyboard: the focused panel goes beside everything in the editor area (`MoveBesideEditor`, VS Code's "to the side") | Drag a tab; View > Panel > Move Next to Editor (Right / Below / Left / Above), also in the palette |
| Add panels | Explorer: one; shown (dock opened) if open, else added to the left. Chat, Terminal, other placeholders: a new numbered tab ("Terminal 2") in the region of its kind, else the default (Chat right, Terminal bottom) | View > Add Panel; a panel's "..." > New <kind> |

Code: `apps/carbon-gui/src/workbench.rs` (area, actions, persistence, `add_panel`, `close_panel`, `close_empty_docks`), `layouts.rs` (presets, store, tests), `test_panel.rs` (placeholder `TestPanel`, title persisted in `PanelInfo::Panel`), `menus.rs` (actions, bindings, dynamic menus), `main.rs` (action wiring, click hook).

Cross-region moves (2026-10-19): how others do it: VS Code drags a terminal into the editor area and arranges it in the editor grid, or "Create New Terminal in Editor Area (to the Side)"; editors never go into the panel ([VS Code terminal](https://code.visualstudio.com/docs/terminal/getting-started)). Zed puts a terminal in a dock or as a center tab ("workspace: new center terminal") ([Zed terminal](https://zed.dev/docs/terminal)). Carbon allows both directions for every panel (the kit's one tree per region already moved panels across; only the drag start of a region's last panel was blocked). Click-tested (window messages, test build; layout file restored after): Chat alone in the right dock dragged right of the editor (dock closed), the editor dragged beside the terminals in the bottom dock, Chat dragged out of the editor area (empty editor appeared), "Panel: Move Next to Editor (Right)" from the palette on a focused terminal, layout restored after restart. Limits: a closed dock is no drop target (open it first, or Ctrl+K arrow); the empty editor has no tab, so it cannot be dragged.

Verified: `cargo check` clean, tests `layout_store_round_trips`, `missing_fields_default` pass. Not verified: everything by hand (T1 started on Windows; T2 not run yet). Known limits: one global layout (no workspaces yet); keyboard move/resize need a focused panel; move into an empty dock untested; auto-hide uses geometry, corners between bottom and side docks may misjudge; dock round-trip UI test blocked (kit `test-support` does not resolve, `STATUS.md` error log).

## 9. Sources (read 2026-10-16)

- VS Code: [Custom Layout](https://code.visualstudio.com/docs/configure/custom-layout), [User interface](https://code.visualstudio.com/docs/editing/userinterface), [issue #95182 perspectives](https://github.com/microsoft/vscode/issues/95182)
- Cursor (forum, A): [layout presets replace Agent/Editor toggle](https://forum.cursor.com/t/agent-editor-toggle-gone-no-way-to-switch-to-editor/150120), [UI resets layout in 2.0](https://forum.cursor.com/t/ui-constantly-resets-layout-in-cursor-2-0/139511?page=3)
- Visual Studio: [Customizing window layouts](https://learn.microsoft.com/en-us/visualstudio/ide/customizing-window-layouts-in-visual-studio), [Arrange and dock windows](https://msdn.microsoft.com/library/z4y0hsax(v=vs.120))
- JetBrains: [Tool windows](https://www.jetbrains.com/help/idea/manipulating-the-tool-windows.html), [View modes](https://www.jetbrains.com/help/idea/viewing-modes.html.md)
- Eclipse: [Moving and docking views](https://help.eclipse.org/latest/topic/org.eclipse.platform.doc.user/tasks/tasks-3e.htm), [Detaching views and editors](https://help.eclipse.org/latest/topic/org.eclipse.platform.doc.user/tasks/tasks-9l.htm)
- Zed: [Visual customization](https://zed.dev/docs/visual-customization), [New panel system](https://zed.dev/blog/new-panel-system), [Windows and projects](https://zed.dev/docs/windows-and-projects.html)
- Sublime: [Sublime forum: groups](https://forum.sublimetext.com/t/how-to-use-sublime-groups/41770), [Origami](https://sublime.wbond.net/packages/Origami)
- Neovim: [winshift.nvim](https://github.com/sindrets/winshift.nvim), [winmove.nvim](https://github.com/MisanthropicBit/winmove.nvim)
- Blender: [Areas manual 5.0](https://docs.blender.org/manual/en/5.0/interface/window_system/areas.html)
- Dear ImGui: [issue #3834](https://github.com/ocornut/imgui/issues/3834), [issue #6217](https://github.com/ocornut/imgui/issues/6217)
- GPUI Kit: `gpui-base-0.7.3/src/dock/{mod,drag}.rs`, `gpui-component-0.7.3/src/dock/` (cargo registry); notes in `project/ui/ui-framework.md` s5
