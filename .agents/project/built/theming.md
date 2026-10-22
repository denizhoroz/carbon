# Theming and fixed styles

Built 2026-10-16. User request: changeable themes as JSON in `style/themes/`, users can add their own; non-changeable styling in one file in `style/`. Location moves to the OS user config dir later.

## What

| Layer | Where | Changeable | Notes |
|---|---|---|---|
| Fixed styles | `apps/carbon-gui/src/style/fixed.rs` | No (code) | Window size 1280x800; logo; title bar gap; command center (32% of the window, 200-520 px, 22 px high); status bar app menu icon 16 px; panel body padding 16 px, gap 8; default dock sizes 240/320/200 px; keyboard resize step 24 px, min 120 px |
| Themes | `apps/carbon-gui/src/style/themes/*.json` | Yes | Kit `ThemeSet` JSON; bundled `carbon.json` = Carbon Dark + Carbon Light; `gruvbox.json` = Gruvbox Dark Hard / Dark / Dark Soft, Light Hard / Light / Light Soft (2026-10-19, morhetz palette, MIT): UI colours, editor syntax colours (`highlight`) and terminal ANSI (`base.*`); any extra `.json` is a custom theme; missing colour keys fall back to kit defaults |
| Theme choice | `apps/carbon-gui/src/style/settings.json` | Yes | `mode`: `dark` (default) / `light` / `system`; `dark_theme` (default **Gruvbox Dark Hard**, user 2026-10-19), `light_theme` (Carbon Light) |
| Terminal background | `terminal_panel/colors.rs` | via theme | The code editor's background (`highlight` `editor.background`, else the kit's input background), so terminal and code are one surface (user 2026-10-19) |
| Loader | `apps/carbon-gui/src/style/theme.rs` | - | `ThemeRegistry::watch_dir` (hot reload of theme files), then sets `dark_theme`/`light_theme` and `Theme::change(mode)` |

## Usage

1. Copy `carbon.json`, rename, edit `name` and colours (keys: `ui-framework.md` s7).
2. Put the name in `settings.json`; restart (settings are read at start; theme files hot-reload).

## Theme keys (kit 0.7.3)

The kit's own `default-theme.json` uses some stale names that its parser silently drops. Real names: `link`, `link.hover`, `link.active`, `drag.border`, `progress.bar.background`, `slider.background`, `description_list.label.background` / `.foreground`; syntax `comment_doc` (not `comment.doc`). The test below fails on any unknown key.

## Verified

- Test `style::theme::tests::theme_files_parse`: every theme file parses with the kit schema, every `colors` / `highlight` / `syntax` key is one the kit knows (round-trip check, added 2026-10-19), and the names in `settings.json` exist (kit would skip a bad file or key with only a log line).
- 2026-10-19 screenshot (test build): Gruvbox Dark Hard; editor and terminal both `#1d2021` (pixel-sampled), bars `#282828`, Gruvbox syntax colours. Light and Soft variants not looked at.
- App starts with no "theme not found" message. Colours not checked by eye.

## Limits

- `system` mode does not follow live OS appearance changes.
- `settings.json` sits in `style/`, not `themes/` (kit reads every `.json` in `themes/` as a theme).
- Contrast contract (`DESIGN.md` s3) not automated; Carbon colours are first picks.
- Fonts are not bundled yet (`DESIGN.md` s3 wants bundled UI and mono fonts).
