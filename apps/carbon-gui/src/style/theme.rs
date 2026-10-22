//! Theme loading. Every `*.json` theme file in `style/themes/` is registered and
//! hot-reloaded by the kit; the Appearance settings pick the mode, the themes
//! and the font sizes, and are re-applied whenever settings change.
//! The themes folder is inside the source tree for now; it moves to the OS
//! user config dir later.

use std::path::PathBuf;

use carbon_core::settings::{Choice, Kind, Setting};
use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry};
use gpui_kit::*;

use crate::settings;

const THEMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/style/themes");

const MODE: &str = "appearance.theme.mode";
const DARK_THEME: &str = "appearance.theme.dark";
const LIGHT_THEME: &str = "appearance.theme.light";
const UI_FONT_SIZE: &str = "appearance.font_size";
const CODE_FONT_FAMILY: &str = "editor.font_family";
const CODE_FONT_SIZE: &str = "editor.font_size";

/// Appearance settings, and the editor's font (the kit theme holds it).
pub fn settings() -> Vec<Setting> {
    vec![
        Setting::new(
            MODE,
            &["Appearance", "Theme"],
            "Color Mode",
            Kind::choice(&[("dark", "Dark"), ("light", "Light"), ("system", "System")]),
            "dark",
        )
        .description("Dark or light colors, or follow the system (read when Carbon starts).")
        .keywords(&["dark mode", "light mode", "appearance"])
        .user_only(),
        // Choices: the theme files of each mode, filled in once they load.
        Setting::new(
            DARK_THEME,
            &["Appearance", "Theme"],
            "Dark Theme",
            Kind::Choice(Vec::new()),
            "Gruvbox Dark Hard",
        )
        .description("Colors used in dark mode.")
        .keywords(&["color theme"])
        .user_only(),
        Setting::new(
            LIGHT_THEME,
            &["Appearance", "Theme"],
            "Light Theme",
            Kind::Choice(Vec::new()),
            "Carbon Light",
        )
        .description("Colors used in light mode.")
        .keywords(&["color theme"])
        .user_only(),
        Setting::new(
            UI_FONT_SIZE,
            &["Appearance", "Text"],
            "Interface Text Size",
            Kind::number(10., 24., 1.),
            16,
        )
        .description("Size of menus, panels and dialogs in pixels; spacing scales with it.")
        .keywords(&["zoom", "scale", "font"])
        .user_only(),
        Setting::new(
            CODE_FONT_FAMILY,
            &["Editor", "Font"],
            "Font Family",
            Kind::Text,
            "",
        )
        .description("Font of the editor and the terminal. Empty uses the default monospace font.")
        .keywords(&["typeface", "monospace"]),
        Setting::new(
            CODE_FONT_SIZE,
            &["Editor", "Font"],
            "Font Size",
            Kind::number(8., 40., 1.),
            13,
        )
        .description("Size of code text in the editor and the terminal, in pixels.")
        .keywords(&["zoom", "text size"]),
    ]
}

/// The kit's own monospace font, used while `editor.font_family` is empty.
struct DefaultMonoFont(SharedString);

impl Global for DefaultMonoFont {}

/// Registers the themes directory and applies the configured themes once
/// loaded, and again on every settings change. Call after `gpui_kit::init`
/// and `settings::init`.
pub fn init(cx: &mut App) {
    cx.set_global(DefaultMonoFont(Theme::global(cx).mono_font_family.clone()));
    if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from(THEMES_DIR), cx, |cx| {
        offer_themes(cx);
        apply(cx);
    }) {
        eprintln!("failed to watch {THEMES_DIR}: {err}");
    }
    cx.observe_global::<settings::AppSettings>(apply).detach();
    apply(cx);
}

/// Fills the theme settings' choices with the loaded theme files.
fn offer_themes(cx: &mut App) {
    let (mut dark, mut light) = (Vec::new(), Vec::new());
    for theme in ThemeRegistry::global(cx).sorted_themes() {
        let choice = Choice {
            value: theme.name.to_string(),
            label: theme.name.to_string(),
        };
        if theme.mode.is_dark() {
            dark.push(choice);
        } else {
            light.push(choice);
        }
    }
    settings::update(cx, |store| {
        store.set_choices(DARK_THEME, dark);
        store.set_choices(LIGHT_THEME, light);
    });
}

fn apply(cx: &mut App) {
    let store = settings::get(cx);
    let mode = store.get_str(MODE);
    let dark_name = SharedString::from(store.get_str(DARK_THEME));
    let light_name = SharedString::from(store.get_str(LIGHT_THEME));
    let ui_size = px(store.get_f64(UI_FONT_SIZE) as f32);
    let code_size = px(store.get_f64(CODE_FONT_SIZE) as f32);
    let family = store.get_str(CODE_FONT_FAMILY);
    let family: SharedString = if family.trim().is_empty() {
        cx.global::<DefaultMonoFont>().0.clone()
    } else {
        family.trim().to_string().into()
    };

    let themes = ThemeRegistry::global(cx).themes();
    let (dark, light) = (
        themes.get(&dark_name).cloned(),
        themes.get(&light_name).cloned(),
    );
    let theme = Theme::global_mut(cx);
    if let Some(dark) = dark {
        theme.dark_theme = dark;
    }
    if let Some(light) = light {
        theme.light_theme = light;
    }

    let mode = match mode.as_str() {
        "light" => ThemeMode::Light,
        "system" => ThemeMode::from(cx.window_appearance()),
        _ => ThemeMode::Dark,
    };
    // `change` reloads the mode's theme, syncs the base layer and refreshes windows.
    Theme::change(mode, None, cx);
    Theme::update(cx, |theme| {
        theme.font_size = ui_size;
        theme.mono_font_size = code_size;
        theme.mono_font_family = family;
    });

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
    use super::{DARK_THEME, LIGHT_THEME, THEMES_DIR, settings};
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
                names.extend(set.themes.into_iter().map(|theme| theme.name.to_string()));
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

        // The default themes exist.
        for setting in settings() {
            if setting.key == DARK_THEME || setting.key == LIGHT_THEME {
                let name = setting.default.as_str().unwrap().to_string();
                assert!(names.contains(&name), "{name:?} missing");
            }
        }
    }
}
