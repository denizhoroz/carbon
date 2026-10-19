//! Theme loading. Every `*.json` theme file in `style/themes/` is registered and
//! hot-reloaded by the kit; `style/settings.json` picks the mode and the themes.
//! Paths are inside the source tree for now; they move to the OS user config dir later.

use std::path::PathBuf;

use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry};
use gpui_kit::*;
use serde::Deserialize;

const THEMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/style/themes");
const SETTINGS_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/style/settings.json");

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Dark,
    Light,
    System,
}

#[derive(Deserialize)]
#[serde(default)]
struct Settings {
    mode: Mode,
    dark_theme: SharedString,
    light_theme: SharedString,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Dark,
            dark_theme: "Gruvbox Dark Hard".into(),
            light_theme: "Carbon Light".into(),
        }
    }
}

fn load_settings() -> Settings {
    let Ok(text) = std::fs::read_to_string(SETTINGS_FILE) else {
        return Settings::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|err| {
        eprintln!("invalid {SETTINGS_FILE}: {err}; using defaults");
        Settings::default()
    })
}

/// Registers the themes directory and applies the configured themes once loaded.
/// Call after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    let settings = load_settings();
    if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from(THEMES_DIR), cx, move |cx| {
        apply(&settings, cx)
    }) {
        eprintln!("failed to watch {THEMES_DIR}: {err}");
    }
}

fn apply(settings: &Settings, cx: &mut App) {
    let themes = ThemeRegistry::global(cx).themes();
    let find = |name: &SharedString| {
        let theme = themes.get(name).cloned();
        if theme.is_none() {
            eprintln!("theme {name:?} not found in {THEMES_DIR}; keeping default");
        }
        theme
    };
    let (dark, light) = (find(&settings.dark_theme), find(&settings.light_theme));

    let theme = Theme::global_mut(cx);
    if let Some(dark) = dark {
        theme.dark_theme = dark;
    }
    if let Some(light) = light {
        theme.light_theme = light;
    }

    let mode = match settings.mode {
        Mode::Dark => ThemeMode::Dark,
        Mode::Light => ThemeMode::Light,
        Mode::System => ThemeMode::from(cx.window_appearance()),
    };
    // `change` reloads the mode's theme, syncs the base layer and refreshes windows.
    Theme::change(mode, None, cx);

    // Editor syntax colours: a theme without its own `highlight` section gets
    // the kit's default for its mode (the kit would keep the light one).
    let theme = Theme::global_mut(cx);
    let config = if mode.is_dark() {
        &theme.dark_theme
    } else {
        &theme.light_theme
    };
    if config.highlight.is_none() {
        theme.highlight_theme = if mode.is_dark() {
            HighlightTheme::default_dark()
        } else {
            HighlightTheme::default_light()
        };
        cx.refresh_windows();
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: the `gpui_kit::*` glob would shadow `#[test]` with GPUI's own macro.
    use super::{THEMES_DIR, load_settings};
    use gpui_kit::component::ThemeSet;

    /// The kit skips invalid theme files with only a log line, so check them here.
    #[test]
    fn theme_files_parse() {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(THEMES_DIR).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let text = std::fs::read_to_string(&path).unwrap();
                let set: ThemeSet = serde_json::from_str(&text)
                    .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
                names.extend(set.themes.into_iter().map(|theme| theme.name));
            }
        }
        // Unknown keys are dropped silently too: every key a theme file sets
        // must survive a parse and re-serialize.
        let mut unknown = Vec::new();
        for entry in std::fs::read_dir(THEMES_DIR).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let raw: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let set: ThemeSet = serde_json::from_value(raw.clone()).unwrap();
            let back = serde_json::to_value(&set).unwrap();
            for (theme, parsed) in raw["themes"]
                .as_array()
                .unwrap()
                .iter()
                .zip(back["themes"].as_array().unwrap())
            {
                for section in [&["colors"][..], &["highlight"], &["highlight", "syntax"]] {
                    let pick = |value: &serde_json::Value| {
                        section.iter().fold(value.clone(), |v, key| v[*key].clone())
                    };
                    let (Some(keys), Some(known)) = (
                        pick(theme).as_object().cloned(),
                        pick(parsed).as_object().cloned(),
                    ) else {
                        continue;
                    };
                    for key in keys.keys() {
                        if !known.contains_key(key) && key != "syntax" {
                            unknown.push(format!("{} {}: {key}", path.display(), theme["name"]));
                        }
                    }
                }
            }
        }

        assert!(
            unknown.is_empty(),
            "not kit theme keys:
{}",
            unknown.join(
                "
"
            )
        );

        let settings = load_settings();
        assert!(
            names.contains(&settings.dark_theme),
            "{:?} missing",
            settings.dark_theme
        );
        assert!(
            names.contains(&settings.light_theme),
            "{:?} missing",
            settings.light_theme
        );
    }
}
