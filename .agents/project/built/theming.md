# Theming and fixed styles

Built 2026-10-16. User request: changeable themes as JSON in `style/themes/`, users can add their own; non-changeable styling in one file in `style/`. Location moves to the OS user config dir later.

## What

| Layer | Where | Changeable | Notes |
|---|---|---|---|
| Fixed styles | `apps/carbon-gui/src/style/fixed.rs` | No (code) | Window size 1280x800; logo; title bar gap; Section Bar 52 px / button 36 / icon 20 / padding 8; panel body padding 16 px, gap 8; default dock sizes 240/320/200 px; keyboard resize step 24 px, min 120 px |
| Themes | `apps/carbon-gui/src/style/themes/*.json` | Yes | Kit `ThemeSet` JSON; bundled `carbon.json` = Carbon Dark + Carbon Light; any extra `.json` is a custom theme; missing colour keys fall back to kit defaults |
| Theme choice | `apps/carbon-gui/src/style/settings.json` | Yes | `mode`: `dark` (default) / `light` / `system`; `dark_theme`, `light_theme` names |
| Loader | `apps/carbon-gui/src/style/theme.rs` | - | `ThemeRegistry::watch_dir` (hot reload of theme files), then sets `dark_theme`/`light_theme` and `Theme::change(mode)` |

## Usage

1. Copy `carbon.json`, rename, edit `name` and colours (keys: `ui-framework.md` s7).
2. Put the name in `settings.json`; restart (settings are read at start; theme files hot-reload).

## Verified

- Test `style::theme::tests::theme_files_parse`: every theme file parses with the kit schema and the names in `settings.json` exist (kit would skip a bad file with only a log line).
- App starts with no "theme not found" message. Colours not checked by eye.

## Limits

- `system` mode does not follow live OS appearance changes.
- `settings.json` sits in `style/`, not `themes/` (kit reads every `.json` in `themes/` as a theme).
- Contrast contract (`DESIGN.md` s3) not automated; Carbon colours are first picks.
- Fonts are not bundled yet (`DESIGN.md` s3 wants bundled UI and mono fonts).
