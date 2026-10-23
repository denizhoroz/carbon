//! Extensions: packages that add languages, grammars, language servers,
//! themes, settings, commands and panels to Carbon without changing its
//! source (`.agents/documents/extensions.md`).
//!
//! A package is a folder with an `extension.toml` (`manifest.rs`) and the
//! files it names. Installed packages live in `installed/<id>/<version>/`
//! (`dirs.rs`), read-only; development folders are linked in place. Flags,
//! grants and install records live in `state.json` (`state.rs`). Install,
//! catalog, signatures and package rules came from the shelved workflow
//! branch. Plain data with no UI types: the GUI turns contributions into
//! panels, menus and highlighters.

mod capability;
mod catalog;
mod dirs;
mod git;
mod install;
mod manifest;
mod package;
mod requires;
mod state;
mod tools;
mod view;
mod wasm;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use capability::{check_capability, describe_capability};
pub use catalog::{
    CATALOG_ENV, CATALOG_FILE, CATALOG_SCHEMA, CATALOG_SIGNATURE, Catalog, CatalogConfig,
    CatalogFiles, CatalogPackage, CatalogVersion, Signing, check_signature, env_url, fetch_catalog,
    fetch_from_catalog,
};
pub use dirs::{DIR_ENV, ExtensionDirs};
pub use git::{FetchControl, GitCheckout, fetch_git, fetch_git_path, is_commit_id};
pub use install::{
    InstallKind, InstallPlan, InstallRecord, InstallSource, MAX_PACKAGE_BYTES, MAX_PACKAGE_FILES,
    Package, Pin, content_hash, installed_hash, list_folder, plan_install, read_package,
    verify_installed,
};
pub use manifest::{
    API_VERSION, Archive, CATEGORIES, CommandDef, Download, ExtensionDef, GrammarDef, LanguageDef,
    LanguageServerDef, MANIFEST, MENUS, MenuDef, PLACEMENTS, PanelDef, PanelKind, SCHEMA_VERSION,
    SettingDef, SettingKind, TARGETS, ThemeDef, current_target,
};
pub use package::{MAX_PACKAGE_PATH, check_package_path, resolve_package_file};
pub use requires::{Missing, RequireKind, Requires};
pub use state::StateFile;
pub use tools::{
    MAX_TOOL_BYTES, ToolProgress, download_host, download_tool, downloaded_program,
    install_tool_file, tool_download,
};
pub use view::{Align, ButtonStyle, ListItem, MAX_VIEW_DEPTH, MAX_VIEW_WIDGETS, Widget};
pub use wasm::{
    CALL_LIMIT, CallError, HostContext, HostEvent, MEMORY_LIMIT, ServerCommand, WasmExtension,
    WasmRuntime,
};

/// Ids under this prefix are Carbon's official extensions: installed only
/// from a catalog signed by a trusted key, or linked for development.
pub const OFFICIAL_PREFIX: &str = "carbon.";

/// An extension's stable id (`publisher.name`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExtensionId(String);

impl ExtensionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Under `carbon.*`.
    pub fn is_official(&self) -> bool {
        self.0.starts_with(OFFICIAL_PREFIX)
    }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lowercase letters, digits and `-`, starting with a letter or digit.
fn is_word(part: &str) -> bool {
    part.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && part
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `publisher.name`: two or more dot-separated words.
fn check_id(id: &str) -> Result<(), String> {
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() >= 2 && parts.iter().all(|part| is_word(part)) {
        Ok(())
    } else {
        Err(format!(
            "id `{id}` must look like `publisher.name` (lowercase letters, digits, `-`)"
        ))
    }
}

/// Text without a leading UTF-8 byte order mark (some Windows editors write one).
fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// Where a loaded extension came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A package in the installed folder (`installed/<id>/<version>/`).
    Installed,
    /// A development folder linked in place (`StateFile::dev`); its files
    /// may change, so nothing is hash-checked.
    Dev,
}

/// One loaded extension.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub def: ExtensionDef,
    pub origin: Origin,
    /// The package folder: every path in the manifest resolves inside it.
    pub dir: PathBuf,
}

impl Loaded {
    /// A file the manifest names, checked to stay inside the package.
    pub fn file(&self, path: &str) -> Result<PathBuf, String> {
        resolve_package_file(&self.dir, path)
    }
}

/// Why an extension is loaded but not in use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// Turned off by the user.
    Disabled,
    /// Turned off by Carbon after it failed (the reason).
    Quarantined(String),
    /// Its files no longer match the install record (the reason).
    Changed(String),
    /// Requirements not met.
    Missing(Vec<Missing>),
}

impl fmt::Display for Blocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("Turned off"),
            Self::Quarantined(why) => write!(f, "Turned off after a failure: {why}"),
            Self::Changed(why) => write!(f, "Turned off: {why}"),
            Self::Missing(missing) => match missing.first() {
                Some(first) => write!(f, "{first}"),
                None => f.write_str("Requirements not met"),
            },
        }
    }
}

/// A manifest that did not load, with why.
#[derive(Clone, Debug)]
pub struct Rejected {
    /// Path of the manifest or package folder.
    pub source: String,
    pub problems: Vec<String>,
}

/// The loaded extensions, sorted by id, and what they contribute. Built from
/// the folders and the state file; rebuilt after every install, removal or
/// flag change (`Registry::load`).
#[derive(Clone, Debug, Default)]
pub struct Registry {
    extensions: Vec<Loaded>,
    blocked: HashMap<ExtensionId, Blocked>,
    rejected: Vec<Rejected>,
    /// (source, warning).
    warnings: Vec<(String, String)>,
}

impl Registry {
    /// Installed packages (newest version of each) and linked development
    /// folders. A development folder wins over an installed package with the
    /// same id. `None` dirs (safe mode): nothing loads.
    pub fn load(dirs: Option<&ExtensionDirs>, state: &StateFile) -> Self {
        let mut registry = Self::default();
        let Some(dirs) = dirs else {
            return registry;
        };
        let mut dev_ids = HashSet::new();
        for folder in &state.dev {
            let manifest = folder.join(MANIFEST);
            let source = manifest.display().to_string();
            match std::fs::read_to_string(&manifest) {
                Ok(text) => {
                    if let Some(id) = registry.add(&source, &text, Origin::Dev, folder, state) {
                        dev_ids.insert(id);
                    }
                }
                Err(err) => registry.reject(&source, vec![format!("cannot read: {err}")]),
            }
        }
        for (path, text) in dirs.installed_manifests() {
            let source = path.display().to_string();
            let Some(dir) = path.parent() else { continue };
            match text {
                Ok(text) => {
                    if let Ok((def, _)) = ExtensionDef::parse(&text)
                        && dev_ids.contains(&def.id)
                    {
                        registry.warn(
                            &source,
                            vec![format!(
                                "`{}` is linked for development; installed copy not used",
                                def.id
                            )],
                        );
                        continue;
                    }
                    registry.add(&source, &text, Origin::Installed, dir, state);
                }
                Err(err) => registry.reject(&source, vec![err]),
            }
        }
        registry.extensions.sort_by(|a, b| a.def.id.cmp(&b.def.id));
        registry.apply_flags(state);
        registry.check_requires();
        registry
    }

    /// Loads manifests given as (folder, text), all as development folders
    /// (tests and previews).
    pub fn from_sources(sources: &[(&Path, &str)]) -> Self {
        let mut registry = Self::default();
        let state = StateFile::default();
        for (dir, text) in sources {
            let source = dir.join(MANIFEST).display().to_string();
            registry.add(&source, text, Origin::Dev, dir, &state);
        }
        registry.extensions.sort_by(|a, b| a.def.id.cmp(&b.def.id));
        registry.check_requires();
        registry
    }

    fn add(
        &mut self,
        source: &str,
        text: &str,
        origin: Origin,
        dir: &Path,
        state: &StateFile,
    ) -> Option<ExtensionId> {
        let (def, warnings) = match ExtensionDef::parse(text) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.reject(source, vec![err]);
                return None;
            }
        };
        let mut problems = def.validate();
        if self.get(&def.id).is_some() {
            problems.push(format!("id `{}` is already loaded", def.id));
        }
        if def.id.is_official()
            && origin == Origin::Installed
            && !state
                .installs
                .get(&def.id)
                .is_some_and(|record| record.signed)
        {
            problems.push(format!(
                "`{}`: `{OFFICIAL_PREFIX}*` extensions install only from a signed catalog",
                def.id
            ));
        }
        problems.extend(def.check_files(dir));
        if !problems.is_empty() {
            self.reject(source, problems);
            return None;
        }
        self.warn(source, warnings);
        let id = def.id.clone();
        self.extensions.push(Loaded {
            def,
            origin,
            dir: dir.to_path_buf(),
        });
        Some(id)
    }

    fn apply_flags(&mut self, state: &StateFile) {
        for loaded in &self.extensions {
            let id = &loaded.def.id;
            let blocked = if let Some(why) = state.quarantined.get(id) {
                Blocked::Quarantined(why.clone())
            } else if let Some(why) = state
                .changed
                .get(id)
                .filter(|_| loaded.origin == Origin::Installed)
            {
                Blocked::Changed(why.clone())
            } else if state.disabled.contains(id) {
                Blocked::Disabled
            } else {
                continue;
            };
            self.blocked.insert(id.clone(), blocked);
        }
    }

    /// Marks every extension whose `[requires]` are not met. One that needs
    /// an extension that is itself blocked is blocked too, so this repeats
    /// until nothing changes.
    fn check_requires(&mut self) {
        loop {
            let found: Vec<(ExtensionId, Vec<Missing>)> = self
                .extensions
                .iter()
                .filter(|loaded| !self.blocked.contains_key(&loaded.def.id))
                .map(|loaded| (loaded.def.id.clone(), self.unmet(&loaded.def)))
                .filter(|(_, missing)| !missing.is_empty())
                .collect();
            if found.is_empty() {
                return;
            }
            for (id, missing) in found {
                self.blocked.insert(id, Blocked::Missing(missing));
            }
        }
    }

    /// `def`'s requirements the loaded, usable extensions do not meet (`def`
    /// need not be loaded: the install dialog asks this of a package).
    pub fn unmet(&self, def: &ExtensionDef) -> Vec<Missing> {
        def.requires.missing(|id| {
            let Some(other) = self.get(&ExtensionId::new(id)) else {
                return Err("not installed".into());
            };
            match self.blocked.get(&other.def.id) {
                Some(why) => Err(format!("installed but not in use ({why})")),
                None => Ok(other.def.version.clone()),
            }
        })
    }

    fn reject(&mut self, source: &str, problems: Vec<String>) {
        self.rejected.push(Rejected {
            source: source.to_string(),
            problems,
        });
    }

    fn warn(&mut self, source: &str, warnings: Vec<String>) {
        self.warnings
            .extend(warnings.into_iter().map(|w| (source.to_string(), w)));
    }

    /// Every loaded extension, blocked ones included.
    pub fn all(&self) -> &[Loaded] {
        &self.extensions
    }

    pub fn get(&self, id: &ExtensionId) -> Option<&Loaded> {
        self.extensions.iter().find(|loaded| &loaded.def.id == id)
    }

    /// Why `id` is loaded but not in use; `None` when it is in use.
    pub fn blocked(&self, id: &ExtensionId) -> Option<&Blocked> {
        self.blocked.get(id)
    }

    /// The extensions in use: loaded and not blocked.
    pub fn active(&self) -> impl Iterator<Item = &Loaded> {
        self.extensions
            .iter()
            .filter(|loaded| !self.blocked.contains_key(&loaded.def.id))
    }

    pub fn is_active(&self, id: &ExtensionId) -> bool {
        self.get(id).is_some() && !self.blocked.contains_key(id)
    }

    pub fn rejected(&self) -> &[Rejected] {
        &self.rejected
    }

    pub fn warnings(&self) -> &[(String, String)] {
        &self.warnings
    }

    /// Ids of the active extensions (to tell what changed after a reload).
    pub fn active_ids(&self) -> BTreeSet<ExtensionId> {
        self.active().map(|loaded| loaded.def.id.clone()).collect()
    }

    /// Languages of active extensions; the first extension (by id) to
    /// declare a language id wins.
    pub fn languages(&self) -> Vec<(&Loaded, &LanguageDef)> {
        let mut seen = HashSet::new();
        self.active()
            .flat_map(|loaded| loaded.def.languages.iter().map(move |lang| (loaded, lang)))
            .filter(|(_, lang)| seen.insert(lang.id.as_str()))
            .collect()
    }

    /// The language for a file: by exact file name first, then by suffix
    /// (longest first, case-insensitive: `d.ts` before `ts`).
    pub fn language_for_path(&self, path: &Path) -> Option<(&Loaded, &LanguageDef)> {
        let name = path.file_name()?.to_str()?;
        let languages = self.languages();
        if let Some(found) = languages
            .iter()
            .find(|(_, lang)| lang.file_names.iter().any(|file| file == name))
        {
            return Some(*found);
        }
        let lower = name.to_ascii_lowercase();
        languages
            .iter()
            .flat_map(|(loaded, lang)| {
                lang.suffixes
                    .iter()
                    .map(move |suffix| (suffix.len(), *loaded, *lang, suffix))
            })
            .filter(|(_, _, _, suffix)| {
                lower
                    .strip_suffix(suffix.as_str())
                    .is_some_and(|rest| rest.ends_with('.'))
            })
            .max_by_key(|(len, ..)| *len)
            .map(|(_, loaded, lang, _)| (loaded, lang))
    }

    /// Grammars of active extensions; the first to declare a name wins.
    pub fn grammars(&self) -> Vec<(&Loaded, &GrammarDef)> {
        let mut seen = HashSet::new();
        self.active()
            .flat_map(|loaded| loaded.def.grammars.iter().map(move |g| (loaded, g)))
            .filter(|(_, grammar)| seen.insert(grammar.name.as_str()))
            .collect()
    }

    /// Language servers of active extensions.
    pub fn language_servers(&self) -> Vec<(&Loaded, &LanguageServerDef)> {
        self.active()
            .flat_map(|loaded| loaded.def.language_servers.iter().map(move |s| (loaded, s)))
            .collect()
    }

    /// Themes of active extensions.
    pub fn themes(&self) -> Vec<(&Loaded, &ThemeDef)> {
        self.active()
            .flat_map(|loaded| loaded.def.themes.iter().map(move |t| (loaded, t)))
            .collect()
    }

    /// Settings of active extensions, as the settings registry takes them.
    pub fn settings(&self) -> Vec<crate::settings::Setting> {
        self.active()
            .flat_map(|loaded| {
                loaded
                    .def
                    .settings
                    .iter()
                    .filter_map(|setting| setting.to_setting(&loaded.def).ok())
            })
            .collect()
    }

    /// Commands of active extensions.
    pub fn commands(&self) -> Vec<(&Loaded, &CommandDef)> {
        self.active()
            .flat_map(|loaded| loaded.def.commands.iter().map(move |c| (loaded, c)))
            .collect()
    }

    /// Panels of active extensions.
    pub fn panels(&self) -> Vec<(&Loaded, &PanelDef)> {
        self.active()
            .flat_map(|loaded| loaded.def.panels.iter().map(move |p| (loaded, p)))
            .collect()
    }

    /// A panel by its global key `<extension id>/<panel id>`.
    pub fn panel(&self, key: &str) -> Option<(&Loaded, &PanelDef)> {
        let (id, panel) = key.split_once('/')?;
        let loaded = self.get(&ExtensionId::new(id))?;
        let def = loaded.def.panels.iter().find(|p| p.id == panel)?;
        Some((loaded, def))
    }

    /// The languages and language servers for the LSP client. `program`
    /// gives a server's program set by the user (it wins over `PATH`); a
    /// copy downloaded into `dirs` is used when neither is found.
    pub fn server_table(
        &self,
        dirs: Option<&ExtensionDirs>,
        program: impl Fn(&Loaded, &LanguageServerDef) -> Option<PathBuf>,
    ) -> crate::lsp::ServerTable {
        use crate::lsp::{LanguageRule, ServerConfig, ServerDownload, ServerTable};
        let languages = self
            .languages()
            .into_iter()
            .map(|(_, lang)| LanguageRule {
                language: lang.id.clone(),
                suffixes: lang
                    .suffixes
                    .iter()
                    .map(|s| s.to_ascii_lowercase())
                    .collect(),
                file_names: lang.file_names.clone(),
            })
            .collect();
        let mut servers: Vec<std::sync::Arc<ServerConfig>> = Vec::new();
        for (loaded, server) in self.language_servers() {
            for language in &server.languages {
                if servers.iter().any(|config| &config.language == language) {
                    continue;
                }
                servers.push(std::sync::Arc::new(ServerConfig {
                    language: language.clone(),
                    name: server.name.clone(),
                    command: server.command.clone(),
                    args: server.args.clone(),
                    env: server.env.clone(),
                    install_hint: server.install_hint.clone().unwrap_or_default(),
                    extension: loaded.def.id.to_string(),
                    server_id: server.id.clone(),
                    program: program(loaded, server),
                    downloaded: dirs.and_then(|dirs| downloaded_program(dirs, loaded, server)),
                    download: tool_download(server).map(|download| ServerDownload {
                        version: download.version.clone(),
                        host: download_host(download),
                    }),
                }));
            }
        }
        ServerTable { languages, servers }
    }

    /// A command by its global key `<extension id>/<command id>`.
    pub fn command(&self, key: &str) -> Option<(&Loaded, &CommandDef)> {
        let (id, command) = key.split_once('/')?;
        let loaded = self.get(&ExtensionId::new(id))?;
        let def = loaded.def.commands.iter().find(|c| c.id == command)?;
        Some((loaded, def))
    }
}
