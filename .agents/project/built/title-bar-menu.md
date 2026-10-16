# Custom title bar and menu bar

Built 2026-10-16. User decisions: classic menu row (not hamburger), custom title bar replacing OS chrome, placeholder items (`DESIGN.md` s4, `STATUS.md` decision log).

## What

| Part | Windows / Linux | macOS |
|---|---|---|
| Title bar | Kit `TitleBar`: own min/max/close, draggable, double-click maximise | Kit `TitleBar` with traffic lights (80 px left pad) |
| Logo | Placeholder "C" on theme `primary` colour, top left | Same |
| Menus | In-window `AppMenuBar` row after the logo; arrows/Esc navigate | Native system menu bar (`cx.set_menus`) |

## Menus

| Menu | Items | Works |
|---|---|---|
| File | Open Folder... (Ctrl+K Ctrl+O), Create Folder..., Close Folder, New File, Save, Exit | Folder items (`workspace-folders.md`) and Exit (Ctrl+Q / Cmd+Q); New File, Save are placeholders |
| Edit | Undo, Redo, Cut, Copy, Paste | None (placeholders) |
| View | Command Palette; Section submenu; dock toggles; auto-hide toggles; Layout submenu; Panel submenu | All except Command Palette (see `section-bar.md`, `panel-layout.md` s8) |
| Help | About | None |

Placeholder items have no handler; they map to unbuilt features in `../not_built/README.md`.

## Code

| File | Role |
|---|---|
| `apps/carbon-gui/src/title_bar.rs` | `AppTitleBar`: logo + `AppMenuBar`; `reload_menus` after a rebuild |
| `apps/carbon-gui/src/menus.rs` | Actions (plain + data actions), key bindings, `app_menus(&MenuState)`, `refresh` (rebuilds native + in-window menus) |
| `apps/carbon-gui/src/main.rs` | Opens the window with `TitleBar::window_options()`; root view holds base focus so menu actions reach handlers |
| `apps/carbon-gui/src/style/fixed.rs` | Logo letter, size, weight; title bar gap |

## Limits

- The kit never fills `AppMenuBar` itself: the app must call `GlobalState::set_app_menus` (done in `menus::refresh`).
- Exact OS behaviour (snap layouts on Windows 11 maximise button hover, Linux decorations) not checked.
- Logo is a placeholder letter, not an icon asset.
