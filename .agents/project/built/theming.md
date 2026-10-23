# Theming and fixed styles

Built 2026-10-16. User request: changeable themes as JSON in `style/themes/`, users can add their own; non-changeable styling in one file in `style/`. Since 2026-10-22 the theme choice is in Settings (`settings.md`), not `style/settings.json`. Since 2026-10-23 (`extensions.md`): Carbon Dark/Light are compiled in, other themes come from extensions (Gruvbox is `carbon.gruvbox`) and the user's `<config>/themes/` folder.

## What

| Layer | Where | Changeable | Notes |
|---|---|---|---|
| Fixed styles | `apps/carbon-gui/src/style/fixed.rs` | No (code) | Window size 1280x800; logo; title bar gap; command center (32% of the window, 200-520 px, 22 px high); status bar app menu icon 16 px; panel body padding 16 px, gap 8; default dock sizes 240/320/200 px; keyboard resize step 24 px, min 120 px |
| Themes | `style/themes/carbon.json` (compiled in), extension `[[themes]]`, `<config>/themes/*.json` | Yes | Kit `ThemeSet` JSON; Carbon Dark + Carbon Light always there; Gruvbox Dark Hard / Dark / Dark Soft, Light Hard / Light / Light Soft in the official extension `carbon.gruvbox` (`carbon-extensions-basic/themes/gruvbox`; 2026-10-19, morhetz palette, MIT): UI colours, editor syntax colours (`highlight`) and terminal ANSI (`base.*`); the first theme of a name wins; missing colour keys fall back to kit defaults |
| Theme choice | Settings > Appearance (user `settings.json`, `settings.md`) | Yes, live | `appearance.theme.mode`: `dark` (default) / `light` / `system`; `appearance.theme.dark` (default Carbon Dark since 2026-10-23, was Gruvbox Dark Hard; a missing theme falls back to Carbon Dark / Light), `appearance.theme.light` (Carbon Light); also `appearance.font_size`, `editor.font_family`, `editor.font_size` |
| Terminal background | `terminal_panel/colors.rs` | via theme | The code editor's background (`highlight` `editor.background`, else the kit's input background), so terminal and code are one surface (user 2026-10-19) |
| Loader | `apps/carbon-gui/src/style/theme.rs` | - | `reload_themes`: `ThemeRegistry::set_custom_themes` (kit patch) with Carbon's, the extensions' and the user folder's sets (polled 1 s; fills the theme dropdowns), then sets `dark_theme`/`light_theme`, `Theme::change(mode)` and the font sizes; runs again on every settings change and after the kit's own registry observer (which re-applies a theme file's `font.size`) |

## Usage

1. Copy `carbon.json` into `<config>/themes/` (`%APPDATA%\Carbon\themes`), rename, edit `name` and colours (keys: `ui-framework.md` s7); or ship it in an extension's `[[themes]]`.
2. Pick it in Settings > Appearance > Theme (applies at once; files in the folder reload within a second).

## Theme keys (kit 0.7.3)

The kit's own `default-theme.json` uses some stale names that its parser silently drops. Real names: `link`, `link.hover`, `link.active`, `drag.border`, `progress.bar.background`, `slider.background`, `description_list.label.background` / `.foreground`; syntax `comment_doc` (not `comment.doc`). The test below fails on any unknown key.

## Verified

- Test `style::theme::tests::theme_files_parse` (Carbon's file, plus `carbon-extensions-basic/themes/*/themes/*.json` when that checkout is in the repo root): every theme file parses with the kit schema, every `colors` / `highlight` / `syntax` key is one the kit knows (round-trip check, added 2026-10-19), and the default theme names in the settings exist (kit would skip a bad file or key with only a log line).
- 2026-10-19 screenshot (test build): Gruvbox Dark Hard; editor and terminal both `#1d2021` (pixel-sampled), bars `#282828`, Gruvbox syntax colours. Light and Soft variants not looked at.
- App starts with no "theme not found" message. Colours not checked by eye.

## Limits

- `system` mode does not follow live OS appearance changes.
- Contrast contract (`DESIGN.md` s3) not automated; Carbon colours are first picks.
- Fonts are not bundled yet (`DESIGN.md` s3 wants bundled UI and mono fonts).
