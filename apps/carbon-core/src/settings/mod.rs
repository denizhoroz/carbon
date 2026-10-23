//! Settings: one typed registry of every option, and the files that set them.
//!
//! Each setting is declared once (key, type, default, label, description,
//! category) by the feature that reads it; the Settings dialog is drawn from
//! the registry. Values live in two JSONC files with flat dotted keys
//! (`"editor.tab_size": 4`): the user's (`user_config_dir()/settings.json`)
//! and the open folder's (`.carbon/settings.json`). A value resolves
//! workspace, then user, then the default; a value of the wrong type or out
//! of range is skipped (the dialog shows it). Keys nobody declared stay in the
//! files untouched. Design: `.agents/project/not_built/settings.md`.

mod jsonc;
pub mod search;

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Map, Value};

use crate::{CoreError, Result};

/// The user settings file's name, in `user_config_dir()`.
pub const USER_FILE: &str = "settings.json";
/// The workspace settings file, relative to the folder's root.
pub const WORKSPACE_FILE: &str = ".carbon/settings.json";

/// Where the user's settings live: `%APPDATA%\Carbon` on Windows,
/// `~/Library/Application Support/Carbon` on macOS, `$XDG_CONFIG_HOME/carbon`
/// (or `~/.config/carbon`) elsewhere. `CARBON_CONFIG_DIR` overrides it (tests,
/// portable setups).
pub fn user_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CARBON_CONFIG_DIR") {
        return Some(dir.into());
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("Carbon"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/Carbon"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|dir| dir.join("carbon"))
    }
}

/// Which file a value is written to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    User,
    Workspace,
}

impl Scope {
    pub const ALL: [Scope; 2] = [Scope::User, Scope::Workspace];

    pub fn label(self) -> &'static str {
        match self {
            Scope::User => "User",
            Scope::Workspace => "Workspace",
        }
    }

    fn other(self) -> Scope {
        match self {
            Scope::User => Scope::Workspace,
            Scope::Workspace => Scope::User,
        }
    }
}

/// One option of a `Kind::Choice`: the value stored and the text shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

/// A setting's type, which decides its control and which values are valid.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Bool,
    /// One of a list of strings. An empty list (not known yet, like theme
    /// names before the themes load) accepts any string.
    Choice(Vec<Choice>),
    Integer {
        min: i64,
        max: i64,
    },
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
    Text,
}

impl Kind {
    pub fn choice(options: &[(&str, &str)]) -> Self {
        Kind::Choice(
            options
                .iter()
                .map(|(value, label)| Choice {
                    value: value.to_string(),
                    label: label.to_string(),
                })
                .collect(),
        )
    }

    pub fn integer(min: i64, max: i64) -> Self {
        Kind::Integer { min, max }
    }

    pub fn number(min: f64, max: f64, step: f64) -> Self {
        Kind::Number { min, max, step }
    }

    /// Whether `value` is a valid value of this kind.
    pub fn accepts(&self, value: &Value) -> bool {
        match self {
            Kind::Bool => value.is_boolean(),
            Kind::Choice(choices) => value.as_str().is_some_and(|value| {
                choices.is_empty() || choices.iter().any(|choice| choice.value == value)
            }),
            Kind::Integer { min, max } => {
                value.as_i64().is_some_and(|n| (*min..=*max).contains(&n))
            }
            Kind::Number { min, max, .. } => {
                value.as_f64().is_some_and(|n| (*min..=*max).contains(&n))
            }
            Kind::Text => value.is_string(),
        }
    }
}

/// The declaration of one setting.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    /// Dotted key, the name in the JSON files (`editor.tab_size`).
    pub key: String,
    /// Path in the category tree, outermost first (`["Editor", "Font"]`).
    pub category: Vec<String>,
    pub label: String,
    /// One plain sentence or two.
    pub description: String,
    pub kind: Kind,
    pub default: Value,
    /// Only the user file may set it (themes, keys: not per folder).
    pub user_only: bool,
    /// Extra words search finds it by.
    pub keywords: Vec<String>,
    /// Takes effect only after a restart.
    pub restart: bool,
    /// The extension that declared it; `None` for Carbon's own.
    pub extension: Option<String>,
}

impl Setting {
    pub fn new(
        key: &str,
        category: &[&str],
        label: &str,
        kind: Kind,
        default: impl Into<Value>,
    ) -> Self {
        Self {
            key: key.into(),
            category: category.iter().map(|part| part.to_string()).collect(),
            label: label.into(),
            description: String::new(),
            kind,
            default: default.into(),
            user_only: false,
            keywords: Vec::new(),
            restart: false,
            extension: None,
        }
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = description.into();
        self
    }

    pub fn keywords(mut self, keywords: &[&str]) -> Self {
        self.keywords = keywords.iter().map(|word| word.to_string()).collect();
        self
    }

    pub fn user_only(mut self) -> Self {
        self.user_only = true;
        self
    }

    pub fn restart(mut self) -> Self {
        self.restart = true;
        self
    }

    fn allowed_in(&self, scope: Scope) -> bool {
        scope == Scope::User || !self.user_only
    }
}

/// When a file was last seen as written, to notice edits made elsewhere.
type Stamp = Option<(SystemTime, u64)>;

fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// One settings file as last read.
struct File {
    path: PathBuf,
    values: Map<String, Value>,
    /// Why the file could not be read; its values are then empty, and the
    /// dialog refuses to write it (it would lose the user's text).
    error: Option<String>,
    stamp: Stamp,
}

impl File {
    fn load(path: PathBuf) -> Self {
        let stamp = stamp(&path);
        let (values, error) = match std::fs::read_to_string(&path) {
            Ok(text) => match jsonc::parse(&text) {
                Ok(values) => (values, None),
                Err(err) => (Map::new(), Some(err)),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => (Map::new(), None),
            Err(err) => (Map::new(), Some(err.to_string())),
        };
        Self {
            path,
            values,
            error,
            stamp,
        }
    }

    fn changed_on_disk(&self) -> bool {
        stamp(&self.path) != self.stamp
    }

    /// Rewrites the file through `edit` (current text in, new text out),
    /// atomically, creating it and its folder if needed.
    fn write(&mut self, edit: impl FnOnce(&str) -> Result<String, String>) -> Result<()> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(CoreError::io(&self.path, err)),
        };
        let text = edit(&text).map_err(|err| {
            CoreError::Settings(format!(
                "{} has errors ({err}); fix it in the file",
                self.path.display()
            ))
        })?;
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(|err| CoreError::io(dir, err))?;
        let mut temp =
            tempfile::NamedTempFile::new_in(dir).map_err(|err| CoreError::io(dir, err))?;
        temp.write_all(text.as_bytes())
            .map_err(|err| CoreError::io(temp.path(), err))?;
        temp.persist(&self.path)
            .map_err(|err| CoreError::io(&self.path, err.error))?;
        self.values = jsonc::parse(&text).unwrap_or_default();
        self.error = None;
        self.stamp = stamp(&self.path);
        Ok(())
    }
}

/// The registry and the user and workspace files.
pub struct Settings {
    definitions: Vec<Setting>,
    index: HashMap<String, usize>,
    user: File,
    workspace: Option<File>,
}

impl Settings {
    /// Reads the user file at `user_path` (missing is fine: all defaults).
    pub fn new(user_path: PathBuf) -> Self {
        Self {
            definitions: Vec::new(),
            index: HashMap::new(),
            user: File::load(user_path),
            workspace: None,
        }
    }

    /// Declares a setting. Order of registration is the order shown; a key
    /// declared again replaces the old declaration in place.
    pub fn register(&mut self, setting: Setting) {
        match self.index.get(&setting.key) {
            Some(&ix) => self.definitions[ix] = setting,
            None => {
                self.index
                    .insert(setting.key.clone(), self.definitions.len());
                self.definitions.push(setting);
            }
        }
    }

    /// Keeps only the declarations `keep` accepts (an extension was turned
    /// off: its settings go). Values in the files stay untouched.
    pub fn retain(&mut self, keep: impl Fn(&Setting) -> bool) {
        self.definitions.retain(|setting| keep(setting));
        self.index = self
            .definitions
            .iter()
            .enumerate()
            .map(|(ix, setting)| (setting.key.clone(), ix))
            .collect();
    }

    /// Replaces the options of a `Choice` setting (lists known only at run
    /// time, like installed themes).
    pub fn set_choices(&mut self, key: &str, choices: Vec<Choice>) {
        if let Some(&ix) = self.index.get(key)
            && let Kind::Choice(old) = &mut self.definitions[ix].kind
        {
            *old = choices;
        }
    }

    pub fn definitions(&self) -> &[Setting] {
        &self.definitions
    }

    pub fn definition(&self, key: &str) -> Option<&Setting> {
        self.definitions.get(*self.index.get(key)?)
    }

    /// Points the workspace scope at folder `root`'s file (`None`: no folder
    /// open). Returns whether anything changed.
    pub fn set_workspace(&mut self, root: Option<&Path>) -> bool {
        let path = root.map(|root| root.join(WORKSPACE_FILE));
        if path.as_ref() == self.workspace.as_ref().map(|file| &file.path) {
            return false;
        }
        self.workspace = path.map(File::load);
        true
    }

    pub fn has_workspace(&self) -> bool {
        self.workspace.is_some()
    }

    /// The file `scope` writes to; `None` for the workspace with no folder open.
    pub fn path(&self, scope: Scope) -> Option<&Path> {
        self.file(scope).map(|file| file.path.as_path())
    }

    /// Whether `path` is one of the settings files (spelled any way: buffers
    /// hold canonical paths).
    pub fn is_settings_file(&self, path: &Path) -> bool {
        let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.into());
        let path = canonical(path);
        Scope::ALL
            .into_iter()
            .filter_map(|scope| self.path(scope))
            .any(|file| canonical(file) == path)
    }

    /// Why `scope`'s file could not be read, if it could not.
    pub fn file_error(&self, scope: Scope) -> Option<&str> {
        self.file(scope)?.error.as_deref()
    }

    fn file(&self, scope: Scope) -> Option<&File> {
        match scope {
            Scope::User => Some(&self.user),
            Scope::Workspace => self.workspace.as_ref(),
        }
    }

    fn file_mut(&mut self, scope: Scope) -> Option<&mut File> {
        match scope {
            Scope::User => Some(&mut self.user),
            Scope::Workspace => self.workspace.as_mut(),
        }
    }

    /// What `scope`'s file says for `key`, valid or not.
    pub fn value_in(&self, scope: Scope, key: &str) -> Option<&Value> {
        self.file(scope)?.values.get(key)
    }

    /// `scope`'s value for `key`, if it is set, allowed there and valid.
    fn valid_in(&self, scope: Scope, setting: &Setting) -> Option<&Value> {
        let value = self.value_in(scope, &setting.key)?;
        (setting.allowed_in(scope) && setting.kind.accepts(value)).then_some(value)
    }

    /// Whether `scope` sets `key` to something that is not used (wrong type,
    /// out of range, or not allowed in that scope).
    pub fn is_invalid_in(&self, scope: Scope, key: &str) -> bool {
        match self.definition(key) {
            Some(setting) => {
                self.value_in(scope, key).is_some() && self.valid_in(scope, setting).is_none()
            }
            None => false,
        }
    }

    /// Where `key`'s value comes from: a scope, or `None` for its default.
    pub fn source(&self, key: &str) -> Option<Scope> {
        let setting = self.definition(key)?;
        [Scope::Workspace, Scope::User]
            .into_iter()
            .find(|&scope| self.valid_in(scope, setting).is_some())
    }

    /// The value in use for `key` (`Null` for an undeclared key).
    pub fn get(&self, key: &str) -> Value {
        let Some(setting) = self.definition(key) else {
            return Value::Null;
        };
        self.source(key)
            .and_then(|scope| self.valid_in(scope, setting))
            .cloned()
            .unwrap_or_else(|| setting.default.clone())
    }

    /// The value for `key` as seen while editing `scope`: the user scope
    /// ignores the workspace file.
    pub fn get_in(&self, scope: Scope, key: &str) -> Value {
        match scope {
            Scope::Workspace => self.get(key),
            Scope::User => match self.definition(key) {
                Some(setting) => self
                    .valid_in(Scope::User, setting)
                    .cloned()
                    .unwrap_or_else(|| setting.default.clone()),
                None => Value::Null,
            },
        }
    }

    pub fn get_bool(&self, key: &str) -> bool {
        self.get(key).as_bool().unwrap_or_default()
    }

    pub fn get_i64(&self, key: &str) -> i64 {
        self.get(key).as_i64().unwrap_or_default()
    }

    pub fn get_f64(&self, key: &str) -> f64 {
        self.get(key).as_f64().unwrap_or_default()
    }

    pub fn get_str(&self, key: &str) -> String {
        self.get(key).as_str().unwrap_or_default().to_string()
    }

    /// Whether the other scope also sets `key` (a badge in the dialog).
    pub fn also_set_in_other(&self, scope: Scope, key: &str) -> bool {
        self.value_in(scope.other(), key).is_some()
    }

    /// Writes `key = value` into `scope`'s file.
    pub fn set(&mut self, scope: Scope, key: &str, value: Value) -> Result<()> {
        let setting = self
            .definition(key)
            .ok_or_else(|| CoreError::Settings(format!("unknown setting {key}")))?;
        if !setting.allowed_in(scope) {
            return Err(CoreError::Settings(format!(
                "{key} can only be set in user settings"
            )));
        }
        if !setting.kind.accepts(&value) {
            return Err(CoreError::Settings(format!(
                "{value} is not a valid value for {key}"
            )));
        }
        let file = self.file_mut(scope).ok_or(CoreError::NoWorkspace)?;
        file.write(|text| jsonc::set(text, key, &value))
    }

    /// Removes `key` from `scope`'s file, so the value falls through to the
    /// next scope or the default.
    pub fn reset(&mut self, scope: Scope, key: &str) -> Result<()> {
        let file = self.file_mut(scope).ok_or(CoreError::NoWorkspace)?;
        if !file.values.contains_key(key) {
            return Ok(());
        }
        file.write(|text| jsonc::remove(text, key))
    }

    /// `scope`'s file, created (as `{}`) if it does not exist yet, to open it
    /// in an editor.
    pub fn ensure_file(&mut self, scope: Scope) -> Result<PathBuf> {
        let file = self.file_mut(scope).ok_or(CoreError::NoWorkspace)?;
        if !file.path.exists() {
            file.write(|_| Ok("{\n}\n".into()))?;
        }
        Ok(file.path.clone())
    }

    /// Whether a file changed on disk since it was last read or written
    /// (cheap: two `stat`s; safe to poll).
    pub fn files_changed(&self) -> bool {
        Scope::ALL
            .into_iter()
            .filter_map(|scope| self.file(scope))
            .any(File::changed_on_disk)
    }

    /// Re-reads files changed on disk since they were last read or written
    /// (edited in the JSON file, another window). Returns whether any was.
    pub fn reload_changed(&mut self) -> bool {
        let mut changed = false;
        for scope in Scope::ALL {
            if let Some(file) = self.file_mut(scope)
                && file.changed_on_disk()
            {
                *file = File::load(file.path.clone());
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn store(dir: &Path) -> Settings {
        let mut settings = Settings::new(dir.join("user").join(USER_FILE));
        settings.register(Setting::new(
            "editor.tab_size",
            &["Editor"],
            "Tab Size",
            Kind::integer(1, 16),
            4,
        ));
        settings.register(
            Setting::new(
                "appearance.theme.mode",
                &["Appearance", "Theme"],
                "Mode",
                Kind::choice(&[("dark", "Dark"), ("light", "Light")]),
                "dark",
            )
            .user_only(),
        );
        settings
    }

    #[test]
    fn workspace_over_user_over_default() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = store(dir.path());
        assert_eq!(settings.get_i64("editor.tab_size"), 4);
        assert_eq!(settings.source("editor.tab_size"), None);

        settings
            .set(Scope::User, "editor.tab_size", json!(2))
            .unwrap();
        assert_eq!(settings.get_i64("editor.tab_size"), 2);
        assert_eq!(settings.source("editor.tab_size"), Some(Scope::User));

        // No folder open: the workspace scope cannot be written.
        assert!(
            settings
                .set(Scope::Workspace, "editor.tab_size", json!(8))
                .is_err()
        );
        let folder = dir.path().join("project");
        assert!(settings.set_workspace(Some(&folder)));
        assert!(!settings.set_workspace(Some(&folder)));
        settings
            .set(Scope::Workspace, "editor.tab_size", json!(8))
            .unwrap();
        assert_eq!(settings.get_i64("editor.tab_size"), 8);
        assert_eq!(settings.get_in(Scope::User, "editor.tab_size"), json!(2));
        assert!(settings.also_set_in_other(Scope::User, "editor.tab_size"));
        assert!(folder.join(WORKSPACE_FILE).exists());

        settings.reset(Scope::Workspace, "editor.tab_size").unwrap();
        assert_eq!(settings.get_i64("editor.tab_size"), 2);
        settings.set_workspace(None);
        assert_eq!(settings.path(Scope::Workspace), None);
    }

    #[test]
    fn invalid_and_disallowed_values_fall_through() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user").join(USER_FILE);
        std::fs::create_dir_all(user.parent().unwrap()).unwrap();
        std::fs::write(&user, r#"{ "editor.tab_size": 99, "unknown.key": [1] }"#).unwrap();
        let folder = dir.path().join("project");
        std::fs::create_dir_all(folder.join(".carbon")).unwrap();
        std::fs::write(
            folder.join(WORKSPACE_FILE),
            r#"{ "appearance.theme.mode": "light" }"#,
        )
        .unwrap();

        let mut settings = store(dir.path());
        settings.set_workspace(Some(&folder));
        assert_eq!(settings.get_i64("editor.tab_size"), 4);
        assert!(settings.is_invalid_in(Scope::User, "editor.tab_size"));
        // User only: the workspace value is ignored.
        assert_eq!(settings.get_str("appearance.theme.mode"), "dark");
        assert!(settings.is_invalid_in(Scope::Workspace, "appearance.theme.mode"));
        assert!(
            settings
                .set(Scope::Workspace, "appearance.theme.mode", json!("light"))
                .is_err()
        );
        // Wrong values are refused; unknown keys survive a write.
        assert!(
            settings
                .set(Scope::User, "editor.tab_size", json!("x"))
                .is_err()
        );
        settings
            .set(Scope::User, "editor.tab_size", json!(3))
            .unwrap();
        let text = std::fs::read_to_string(&user).unwrap();
        assert!(text.contains("\"unknown.key\": [1]"));
    }

    #[test]
    fn broken_file_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user").join(USER_FILE);
        std::fs::create_dir_all(user.parent().unwrap()).unwrap();
        std::fs::write(&user, "{ \"editor.tab_size\": ").unwrap();
        let mut settings = store(dir.path());
        assert!(settings.file_error(Scope::User).is_some());
        assert!(
            settings
                .set(Scope::User, "editor.tab_size", json!(2))
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&user).unwrap(),
            "{ \"editor.tab_size\": "
        );
    }

    #[test]
    fn reloads_files_changed_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = store(dir.path());
        settings
            .set(Scope::User, "editor.tab_size", json!(2))
            .unwrap();
        assert!(!settings.reload_changed());
        let user = settings.path(Scope::User).unwrap().to_path_buf();
        std::fs::write(&user, "{ \"editor.tab_size\": 12 }").unwrap();
        assert!(settings.reload_changed());
        assert_eq!(settings.get_i64("editor.tab_size"), 12);
    }

    #[test]
    fn choices_can_change_and_empty_accepts_any() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = store(dir.path());
        settings.register(Setting::new(
            "appearance.theme.dark",
            &["Appearance"],
            "Dark Theme",
            Kind::Choice(Vec::new()),
            "A",
        ));
        assert!(
            settings
                .set(Scope::User, "appearance.theme.dark", json!("Anything"))
                .is_ok()
        );
        settings.set_choices(
            "appearance.theme.dark",
            vec![Choice {
                value: "A".into(),
                label: "A".into(),
            }],
        );
        assert!(settings.is_invalid_in(Scope::User, "appearance.theme.dark"));
        assert_eq!(settings.get_str("appearance.theme.dark"), "A");
    }
}
