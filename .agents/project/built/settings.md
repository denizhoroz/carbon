# Settings

Built 2026-10-22 (ST-01, first part). User request: a sophisticated Settings menu with a **search bar on top** and a **tree of setting categories on the left**. Opened from Settings in the status bar app menu, File > Preferences > Settings, the palette ("Preferences: Settings") and **Ctrl+,** (VS Code's key).

## 1. Decisions (user, 2026-10-22)

| # | Topic | Decision |
|---|---|---|
| 1 | Layout | Search bar on top, category tree on the left (user requirement) |
| 2 | Where it opens | **Modal dialog** |
| 3 | Search | **Filter**: the list keeps only matching settings with the matched text highlighted; the tree keeps only matching branches with a count each |
| 4 | Scopes | **User + Workspace** in the first version |
| 5 | Right pane | **One long scroll** with category titles; clicking a category scrolls there; the tree follows the scroll |

Everything else below is my design.

## 2. How others do it (research 2026-10-22)

| App | Opens as | Search | Tree | Scopes | Raw file |
|---|---|---|---|---|---|
| VS Code | Editor tab | Top, full width; `@modified`, `@ext:`, `@lang:`, `@id:`, `@feature:`; funnel button | Table of contents, ~3 levels, "Commonly Used" first; narrows to matching categories | User / Workspace / Folder tabs | `settings.json` via a button |
| Zed (GPUI, Oct 2025) | Separate window | Search + nav bar | Left nav | User / project / server | `settings.json` kept "for unfettered control" |
| JetBrains | Modal dialog | Top left above the tree; matches highlighted | Deep tree | Project settings marked with an icon | None |

- Zed: settings as strong types that map to the files; UI metadata through macros fought the crate graph and was dropped; "unset" differs from "set to nothing".
- Kit `gpui_kit::component::setting::Settings` is **not used**: its search sits in the sidebar, its tree is 2 levels, no scope switch or counts. Its field kinds were the model.

Sources: [VS Code settings](https://code.visualstudio.com/docs/configure/settings), [How We Rebuilt Settings in Zed](https://zed.dev/blog/settings-ui), [Zed 0.208.4](https://zed.dev/releases/stable/0.208.4), [Zed 0.210.4](https://zed.dev/releases/stable/0.210.4), [IntelliJ Settings dialog](https://www.jetbrains.com/help/idea/settings-preferences-dialog.html)

## 3. What

```
┌ Settings ───────────────────────────────────────────────────────── x ┐
│ [🔍 Search settings (@modified, @id:, @ext:)   3 settings] User Workspace {} │
├──────────────────┬───────────────────────────────────────────────────┤
│ ▾ Appearance     │ Appearance › Theme                                │
│     Theme        │ ▌Color Mode          [ Dark            ▾] ↶ ⋯     │
│     Text         │   Dark Theme         [ Gruvbox Dark Hard ▾]  ⋯     │
│ ▾ Editor         │ Editor › Font                                     │
│     Font  ...    │   Font Size          [ - 13 + ]               ⋯   │
│   Explorer       │                                                   │
└──────────────────┴───────────────────────────────────────────────────┘
```

| Part | Behaviour |
|---|---|
| Dialog | 80% x 85% of the window, at most 1100 x 760, at least 640 x 420; Esc or x closes. Changes save as they are made (no OK) |
| Top bar | Search box across the width (focused on open; count of matches inside it; x clears); scope switch **User / Workspace** (Workspace disabled with a tip when no folder is open); `{}` closes the dialog and opens the scope's JSON file in an editor tab (created as `{}` if missing) |
| Tree | Categories of any depth, in the order their first setting was registered. Click: scroll the list there (and unfold); the chevron folds. Highlight: the category at the top of the list (or the one clicked, until the list is scrolled by hand). Keys when focused (click or Tab): Up/Down move, Right unfolds or goes to the first child, Left folds or goes to the parent, Up from the top returns to the search box |
| List | One virtual list: a title per category holding settings (`Editor › Font`), then its rows. Row: label, description, notes, control on the right; a bar on the left marks a value this scope sets; reset button (↶) when it does; `⋯` menu: Copy Setting ID, Copy Setting as JSON, Reset Setting |
| Notes on a row | "Can only be set in User settings." (user-only setting in the Workspace scope, control disabled); "Overridden by Workspace settings." / "Also set in User settings."; invalid value in this scope (red); "Applies after a restart." (none yet) |
| Controls | Bool: switch. Choice: dropdown (scrollable). Integer / number: number box with - / +, applied while typing when in range, clamped on Enter or leaving the box. Text: box applied on Enter or leaving it ("Default" placeholder when the default is empty) |
| Search | Every word must appear (any case) in the label, description, key, category or keywords; matches highlighted in label and description. Filters: `@modified` (set in the current scope), `@id:<key or prefix>`, `@ext:<extension id>` (no extension settings exist yet). No match: "No settings match" + Clear Search. Clearing returns the list to where it was |
| File with errors | A red banner above the list ("... settings file has errors, so its values are not used: <parser message>") with Open File; the dialog refuses to write that file (it would lose the user's text) |

## 4. Data

| Part | Where | Notes |
|---|---|---|
| Registry + files | core `settings/mod.rs` (`Settings`, `Setting`, `Kind`, `Scope`) | A setting: key, category path, label, description, kind (bool, choice, integer, number, text), default, user only, keywords, restart, extension. Declared by the feature that reads it (`style::theme::settings()`, `editor_panel::settings()`, `explorer_panel::settings()`), registered in `settings::init` |
| Files | User: `%APPDATA%\Carbon\settings.json` (macOS `~/Library/Application Support/Carbon/`, else `$XDG_CONFIG_HOME/carbon/` or `~/.config/carbon/`; `CARBON_CONFIG_DIR` overrides). Workspace: `<folder>/.carbon/settings.json` | Flat dotted keys (`"editor.tab_size": 4`); JSONC (comments, trailing commas) |
| Resolution | default < user < workspace | A value of the wrong type, out of range, or a user-only key in the workspace file is skipped (falls through) and shown as invalid. Missing key = unset |
| Writing | core `settings/jsonc.rs` | Changes one top-level key in place: comments, order and other keys (also unknown ones) survive; written atomically (temp file + rename). Reset removes the key (and its line) |
| Live | gui `settings.rs` | `AppSettings` GPUI global; features observe it. Files are polled every second (two `stat`s) for edits made elsewhere (the JSON tab, an editor, another program) and reloaded |
| Search / outline | core `settings/search.rs`; gui `settings/tree.rs` | Query parsing and matching in the core (tested); tree and list built together so a category maps to a list position |
| View | gui `settings/view.rs` (dialog, top bar, tree, list), `settings/row.rs` (rows, controls) | Sizes in `style/fixed.rs` (`SETTINGS_*`) |

## 5. Settings so far

Each key exists only because the feature reads it; defaults equal what Carbon did before.

| Category | Key | Kind, default | Applies |
|---|---|---|---|
| Appearance > Theme | `appearance.theme.mode` | dark / light / system, `dark`; user only | Live |
| Appearance > Theme | `appearance.theme.dark` / `.light` | Choice of loaded theme files of that mode, `Gruvbox Dark Hard` / `Carbon Light`; user only | Live |
| Appearance > Text | `appearance.font_size` | 10-24, `16`; user only | Live (kit rem size) |
| Editor > Font | `editor.font_family` | Text, empty = kit's default monospace | Live, editor and terminal |
| Editor > Font | `editor.font_size` | 8-40, `13` | Live, editor and terminal |
| Editor > Indentation | `editor.tab_size`, `editor.insert_spaces` | 1-16 `2`; `true` | Live, open editors |
| Editor > Display | `editor.soft_wrap`, `editor.line_numbers`, `editor.indent_guides`, `editor.show_whitespace` | `false`, `true`, `true`, `false` | Live, open editors |
| Explorer | `explorer.confirm_delete` | `true` | Next delete |

`apps/carbon-gui/src/style/settings.json` (theme choice) is gone: its values were the defaults above, and theme choice now lives in the user file.

## 6. Verified (2026-10-22)

- Tests: core `settings::{tests, jsonc::tests, search::tests}` (resolution order, invalid / user-only fall through, broken file not overwritten, reload after outside edit, comment-keeping edits, search and filters); gui `settings::tree::tests`, `settings::row::tests`, `style::theme::tests::theme_files_parse` (default theme names exist). Clippy clean, 36 GUI tests.
- Click-tested (test build, `CARBON_CONFIG_DIR` set to a scratch folder, real mouse input): opened from the status bar menu; search "size" (3 rows, highlights, counts); + on Font Size wrote the file, bar and reset showed, terminal text grew; a hand edit of the file (comment, trailing comma, `light` mode) reloaded live; Workspace scope: Line Numbers off wrote `.carbon/settings.json` and the open editor lost its numbers; user-only rows disabled with the note there; "Overridden by Workspace settings." in User scope; reset kept the file's comment; tree click scrolled and highlighted; wheel scrolling moved the tree highlight; Up/Down/Left in the tree; `{}` opened the JSON in an editable tab; a broken file showed the banner; Dark Theme dropdown switched the theme live.
- Found and fixed while testing: the user file opened read-only (buffers hold canonical paths; the check now canonicalizes both sides).
- Not checked: Ctrl+, (modifier keys cannot be sent by the test method), macOS / Linux paths, a narrow window (rows wrap their control under the text).

## 7. Limits

- The settings file open in an editor tab is not watched for outside changes (it is outside the folder); the dialog writing it while the tab has unsaved edits leaves a conflict on save.
- No "Show advanced" yet (no advanced settings); no per-language settings (`@lang:`); no folder scope (multi-root does not exist).
- Workspace settings are trusted as is (no workspace trust prompt yet).
- `system` mode still does not follow live OS appearance changes (`theming.md`).
- Settings with lists or objects (Kind list/map) do not exist yet; such keys can only be edited in JSON.
- Next: more settings as features read them (terminal font/scrollback/cursor, auto save, telemetry when it exists), settings in the palette, VS Code settings import (ST-06), keymap editor (ST-04).
