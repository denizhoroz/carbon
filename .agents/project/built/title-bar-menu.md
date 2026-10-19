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
| File | Open Folder... (Ctrl+K Ctrl+O), Create Folder..., Close Folder, New File (Ctrl+N), Save (Ctrl+S), Exit (Ctrl+Q) | All work: folder items (`workspace-folders.md`); New File, Save and Exit (asks about unsaved files) in `code-editor.md` |
| Edit | Undo, Redo, Cut, Copy, Paste | The kit's text actions: act on the focused editor or text box; in the Explorer, on files (2026-10-17) |
| View | Command Palette (Ctrl+Shift+P, F1), Go to File... (Ctrl+P), Go to Line... (Ctrl+G); Section submenu; dock toggles; auto-hide toggles; Layout submenu; Panel submenu (Move to dock / center, Move Next to Editor right/below/left/above) | All (palette: `command-palette.md`; see `section-bar.md`, `panel-layout.md` s8) |
| Help | About | None |

Placeholder items have no handler; they map to unbuilt features in `../not_built/README.md`.

## Code

| File | Role |
|---|---|
| `apps/carbon-gui/src/title_bar.rs` | `AppTitleBar`: logo + `AppMenuBar`; `reload_menus` after a rebuild |
| `apps/carbon-gui/src/menus.rs` | Actions (plain + data actions), key bindings, `app_menus(&MenuState)`, `refresh` (rebuilds native + in-window menus) |
| `apps/carbon-gui/src/main.rs` | Opens the window with `TitleBar::window_options()`; root view holds base focus so menu actions reach handlers |
| `apps/carbon-gui/src/style/fixed.rs` | Logo letter, size, weight; title bar gap; `BAR_HEIGHT` (30 px, title and status bar, = tab bar; 2026-10-17) |

## Limits

- The kit never fills `AppMenuBar` itself: the app must call `GlobalState::set_app_menus` (done in `menus::refresh`).
- Exact OS behaviour (snap layouts on Windows 11 maximise button hover, Linux decorations) not checked.
- Logo is a placeholder letter, not an icon asset.
