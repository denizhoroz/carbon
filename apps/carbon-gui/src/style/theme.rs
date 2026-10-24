//! Theme loading. Themes come from Carbon's own file (Carbon Dark and Light,
//! compiled in), the active extensions, and the user's `themes/` folder in
//! the config dir (polled, so a saved file shows at once); the first theme of
//! a name wins. The Appearance settings pick the mode, the themes and the font
//! sizes, and are re-applied whenever settings change.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use carbon_core::settings::{Choice, Kind, Setting};
use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry};
use gpui_kit::*;

use crate::settings;

/// Carbon's own themes, always there.
const CARBON_THEMES: &str = include_str!("themes/carbon.json");
/// The themes used when the chosen one is not installed.
const FALLBACK_DARK: &str = "Carbon Dark";
const FALLBACK_LIGHT: &str = "Carbon Light";
/// How often the user's themes folder is checked for changes.
const POLL: Duration = Duration::from_secs(1);

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
        // Choices: the themes of each mode, filled in once they load.
        Setting::new(
            DARK_THEME,
            &["Appearance", "Theme"],
            "Dark Theme",
            Kind::Choice(Vec::new()),
            FALLBACK_DARK,
        )
        .description("Colors used in dark mode. Extensions add more themes.")
        .keywords(&["color theme"])
        .user_only(),
        Setting::new(
            LIGHT_THEME,
            &["Appearance", "Theme"],
            "Light Theme",
            Kind::Choice(Vec::new()),
            FALLBACK_LIGHT,
        )
        .description("Colors used in light mode. Extensions add more themes.")
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

/// The user's themes folder as last read.
#[derive(Default)]
struct UserThemes {
    stamp: Vec<(PathBuf, Option<SystemTime>, u64)>,
}

impl Global for UserThemes {}

/// Loads the themes and applies the configured ones, again on every
/// settings change and whenever the user's themes folder changes. Call after
/// `gpui_kit::init`, `settings::init` and `extensions::init`.
pub fn init(cx: &mut App) {
    cx.set_global(DefaultMonoFont(Theme::global(cx).mono_font_family.clone()));
    cx.set_global(UserThemes::default());
    reload_themes(cx);
    cx.observe_global::<settings::AppSettings>(apply).detach();
    // The kit re-applies the active theme file whenever the registry changes
    // (its own observer, registered first), which puts the file's font size
    // back; this one runs after it and puts the settings back.
    cx.observe_global::<ThemeRegistry>(apply).detach();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL).await;
            let changed = cx.update(|cx| cx.global::<UserThemes>().stamp != user_themes_stamp());
            if changed {
                cx.update(reload_themes);
            }
        }
    })
    .detach();
}

/// `<config>/themes`: theme files the user adds by hand.
fn user_themes_dir() -> Option<PathBuf> {
    carbon_core::settings::user_config_dir().map(|dir| dir.join("themes"))
}

/// The theme files in the user's folder, with their times and sizes.
fn user_themes_stamp() -> Vec<(PathBuf, Option<SystemTime>, u64)> {
    let Some(dir) = user_themes_dir() else {
        return Vec::new();
    };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
                .map(|path| {
                    let meta = std::fs::metadata(&path).ok();
                    let modified = meta.as_ref().and_then(|meta| meta.modified().ok());
                    let len = meta.map_or(0, |meta| meta.len());
                    (path, modified, len)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Reads every theme source again (extensions changed, a user file
/// changed) and applies the configured themes.
pub fn reload_themes(cx: &mut App) {
    if !cx.has_global::<UserThemes>() {
        // Extensions load before `init`, which reads them.
        return;
    }
    let mut sources: Vec<(String, String)> = vec![("Carbon".into(), CARBON_THEMES.into())];
    let registry = crate::extensions::registry(cx);
    for (loaded, theme) in registry.themes() {
        let text = loaded.file(&theme.file).and_then(|path| {
            std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))
        });
        match text {
            Ok(text) => sources.push((loaded.def.id.to_string(), text)),
            Err(err) => eprintln!("theme of {}: {err}", loaded.def.id),
        }
    }
    let stamp = user_themes_stamp();
    for (path, _, _) in &stamp {
        match std::fs::read_to_string(path) {
            Ok(text) => sources.push((path.display().to_string(), text)),
            Err(err) => eprintln!("theme {}: {err}", path.display()),
        }
    }
    cx.global_mut::<UserThemes>().stamp = stamp;
    let errors = ThemeRegistry::global_mut(cx).set_custom_themes(
        sources
            .iter()
            .map(|(source, text)| (source.as_str(), text.as_str())),
    );
    for error in errors {
        eprintln!("theme file skipped: {error}");
    }
    offer_themes(cx);
    apply(cx);
}

/// Fills the theme settings' choices with the loaded themes.
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

    // A chosen theme that is not installed (its extension was removed)
    // falls back to Carbon's own.
    let themes = ThemeRegistry::global(cx).themes();
    let (dark, light) = (
        themes
            .get(&dark_name)
            .or_else(|| themes.get(FALLBACK_DARK))
            .cloned(),
        themes
            .get(&light_name)
            .or_else(|| themes.get(FALLBACK_LIGHT))
            .cloned(),
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
    use super::{CARBON_THEMES, DARK_THEME, LIGHT_THEME, settings};
    use gpui_kit::component::ThemeSet;

    /// Carbon's own theme file and, when a checkout of the official
    /// extensions repo is in the repo root (`carbon-extensions-basic/`,
    /// ignored by git), its theme files (`themes/*/themes/*.json`).
    fn theme_files() -> Vec<(String, String)> {
        let mut files = vec![("carbon.json".to_string(), CARBON_THEMES.to_string())];
        let root = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../carbon-extensions-basic/themes"
        );
        for extension in std::fs::read_dir(root).into_iter().flatten() {
            let themes = extension.unwrap().path().join("themes");
            for entry in std::fs::read_dir(themes).into_iter().flatten() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    files.push((path.display().to_string(), text));
                }
            }
        }
        files
    }

    /// The kit skips invalid theme files with only a log line, so check them here.
    #[test]
    fn theme_files_parse() {
        // Unknown keys are dropped silently too: every key a theme file sets
        // must survive a parse and re-serialize.
        let mut unknown = Vec::new();
        let files = theme_files();
        for (path, text) in &files {
            let raw: serde_json::Value = serde_json::from_str(text).unwrap();
            let set: ThemeSet =
                serde_json::from_value(raw.clone()).unwrap_or_else(|err| panic!("{path}: {err}"));
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
                            unknown.push(format!("{path} {}: {key}", theme["name"]));
                        }
                    }
                }
            }
        }
        assert!(
            unknown.is_empty(),
            "not kit theme keys:\n{}",
            unknown.join("\n")
        );

        // The default themes are Carbon's own, always there.
        let own: ThemeSet = serde_json::from_str(CARBON_THEMES).unwrap();
        let own: Vec<String> = own
            .themes
            .iter()
            .map(|theme| theme.name.to_string())
            .collect();
        for setting in settings() {
            if setting.key == DARK_THEME || setting.key == LIGHT_THEME {
                let name = setting.default.as_str().unwrap().to_string();
                assert!(own.contains(&name), "{name:?} missing");
            }
        }
    }
}
