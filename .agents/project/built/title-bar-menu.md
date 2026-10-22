# Custom title bar and menu bar

Built 2026-10-16. User decisions: classic menu row, custom title bar replacing OS chrome, placeholder items (`DESIGN.md` s4, `STATUS.md` decision log). 2026-10-22 (appearance rework, user): the Section Bar is gone; the title bar gained the **command center** (VS Code-style search box that opens the command palette, so people see it exists). The **app menu** button (three lines: Extensions, Settings, the palette) is at the status bar's right end (`status-bar.md`).

## What

| Part | Windows / Linux | macOS |
|---|---|---|
| Title bar | Kit `TitleBar`: own min/max/close, draggable, double-click maximise | Kit `TitleBar` with traffic lights (80 px left pad) |
| Logo | Placeholder "C" on theme `primary` colour, top left | Same |
| Menus | In-window `AppMenuBar` row after the logo; arrows/Esc navigate | Native system menu bar (`cx.set_menus`) |
| Command center (2026-10-22) | Box centered on the window (where the palette opens): search icon, "Search commands and files", the palette shortcut (`Kbd::binding_for_action`, shows F1). Click opens the palette in command mode (`>`). Width: 32% of the window, 200-520 px, 300 px kept free each side for the menus and buttons; hidden when that leaves under 200 px. Hover lightens it. It `occlude`s the title bar under it: otherwise the bar's mouse-down starts a window drag and a real click never opens the palette | Same |

## Menus

| Menu | Items | Works |
|---|---|---|
| File | Open Folder... (Ctrl+K Ctrl+O), Create Folder..., Close Folder, New File (Ctrl+N), Save (Ctrl+S), Preferences > Extensions / Settings (2026-10-22), Exit (Ctrl+Q) | All work: folder items (`workspace-folders.md`); New File, Save and Exit (asks about unsaved files) in `code-editor.md`; Preferences open the Manage dialogs: Extensions a placeholder, Settings the Settings dialog (`settings.md`, Ctrl+,; also in the palette as "Preferences: ...") |
| Edit | Undo, Redo, Cut, Copy, Paste | The kit's text actions: act on the focused editor or text box; in the Explorer, on files (2026-10-17) |
| View | Command Palette (Ctrl+Shift+P, F1), Go to File... (Ctrl+P), Go to Line... (Ctrl+G); dock toggles; auto-hide toggles; Layout submenu; Panel submenu (Move to dock / center, Move Next to Editor right/below/left/above) | All (palette: `command-palette.md`; `panel-layout.md` s8) |
| Help | About | None |

Placeholder items have no handler; they map to unbuilt features in `../not_built/README.md`.

## Code

| File | Role |
|---|---|
| `apps/carbon-gui/src/title_bar.rs` | `AppTitleBar`: logo + `AppMenuBar`, `command_center` (absolutely placed over the kit `TitleBar`, `command_center_width`, test); `reload_menus` after a rebuild |
| `apps/carbon-gui/src/manage.rs` | `Manage` (Extensions, Settings): label, icon, action (`ManageExtensions` / `ManageSettings`), `open` = Settings dialog (`settings.rs`) or the Extensions placeholder |
| `apps/carbon-gui/src/menus.rs` | Actions (plain + data actions), key bindings, `app_menus(&MenuState)`, `refresh` (rebuilds native + in-window menus) |
| `apps/carbon-gui/src/main.rs` | Opens the window with `TitleBar::window_options()`; root view holds base focus so menu actions reach handlers |
| `apps/carbon-gui/src/style/fixed.rs` | Logo letter, size, weight; title bar gap; `BAR_HEIGHT` (30 px, title and status bar, = tab bar; 2026-10-17); `COMMAND_CENTER_*` |

## Verified (2026-10-22)

Clippy, fmt, 33 GUI tests. Click-tested (test build, `target/carbon-gui` JSON restored) with **real mouse input** (`SetCursorPos` + `mouse_event`; window messages sent to the client area skip the title bar's hit test and drag handler and hid the first bug): the command center opens the palette with `>`; "Preferences: Extensions / Settings" in the palette; at 860 px the box shrinks and stays clear of the menus. An old `layout.json` (`{active, sections: {code}}`) loaded and was rewritten as `{dock}`. Not checked: dragging the window by the empty title bar beside the box, macOS.

## Limits

- The kit never fills `AppMenuBar` itself: the app must call `GlobalState::set_app_menus` (done in `menus::refresh`).
- Exact OS behaviour (snap layouts on Windows 11 maximise button hover, Linux decorations) not checked.
- Logo is a placeholder letter, not an icon asset.
- The command center shows F1 (the binding GPUI ranks first), not Ctrl+Shift+P; it does not show the workspace name (VS Code does).
