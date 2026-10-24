//! `extension.toml`, schema 1 (`.agents/documents/extensions.md` s4).
//!
//! ```toml
//! id = "carbon.rust"
//! name = "Rust"
//! version = "0.1.0"
//! schema_version = 1
//! carbon_api = ">=0.1, <0.2"
//! icon = "icon.png"
//! categories = ["language"]
//! capabilities = ["process.exec:rust-analyzer", "net:github.com"]
//!
//! [[languages]]
//! id = "rust"
//! name = "Rust"
//! suffixes = ["rs"]
//! grammar = "rust"
//! line_comment = "//"
//!
//! [[grammars]]
//! name = "rust"
//! wasm = "grammars/rust.wasm"
//! highlights = "grammars/rust/highlights.scm"
//!
//! [[language_servers]]
//! id = "rust-analyzer"
//! name = "rust-analyzer"
//! languages = ["rust"]
//! command = "rust-analyzer"
//! [language_servers.download.windows-x64]
//! url = "https://github.com/.../rust-analyzer-x86_64-pc-windows-msvc.zip"
//! sha256 = "<64 hex>"
//! archive = "zip"
//! bin = "rust-analyzer.exe"
//! ```
//!
//! Every file is named by a package path (`package.rs` rules). Running a
//! language server needs `process.exec:<command>` in `capabilities`, and a
//! download needs `net:<host>`, so the install prompt shows both.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use super::{
    ExtensionId, Requires, Widget, check_capability, check_id, check_package_path, is_word,
    resolve_package_file, strip_bom, view,
};
use crate::graph::{FieldKind, FieldSpec, GraphSpec, NodeTypeSpec, check_graph_id};
use crate::settings::{Choice, Kind, Setting};

/// The manifest's file name, at the package root.
pub const MANIFEST: &str = "extension.toml";
/// Manifest format version this build reads.
pub const SCHEMA_VERSION: u32 = 1;
/// Host contracts this build offers; a manifest's `carbon_api` range must
/// match one (`since_v0.1.0`, `since_v0.2.0` in `wit/`).
pub const SUPPORTED_APIS: [Version; 2] = [Version::new(0, 1, 0), Version::new(0, 2, 0)];
/// The newest host contract.
pub const API_VERSION: Version = Version::new(0, 2, 0);
/// The first contract with agents, graphs and node types.
const GRAPHS_API: Version = Version::new(0, 2, 0);
/// Values of `categories` (Manage Extensions filters by them).
pub const CATEGORIES: [&str; 6] = ["language", "theme", "tool", "panel", "agent", "other"];
/// Top-level menus an extension may add items to.
pub const MENUS: [&str; 4] = ["File", "Edit", "View", "Help"];
/// Where a panel opens first.
pub const PLACEMENTS: [&str; 4] = ["center", "left", "right", "bottom"];
/// File types an extension icon may be (lowercase extensions).
pub const ICON_TYPES: [&str; 5] = ["png", "svg", "jpg", "jpeg", "webp"];
/// Largest icon file; a larger one is not shown (the placeholder is).
pub const MAX_ICON_BYTES: u64 = 256 * 1024;

/// Problems with an icon path's file type (`ICON_TYPES`).
pub fn check_icon_type(path: &str) -> Result<(), String> {
    let kind = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    match kind {
        Some(kind) if ICON_TYPES.contains(&kind.as_str()) => Ok(()),
        _ => Err(format!(
            "icon `{path}` must be one of {}",
            ICON_TYPES.join(", ")
        )),
    }
}

/// Download targets: `<os>-<cpu>`.
pub const TARGETS: [&str; 6] = [
    "windows-x64",
    "windows-arm64",
    "macos-x64",
    "macos-arm64",
    "linux-x64",
    "linux-arm64",
];

/// This build's download target, if it is one of `TARGETS`.
pub fn current_target() -> Option<&'static str> {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        _ => return None,
    };
    let cpu = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return None,
    };
    TARGETS
        .iter()
        .find(|target| **target == format!("{os}-{cpu}"))
        .copied()
}

/// One `extension.toml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtensionDef {
    pub id: ExtensionId,
    pub schema_version: u32,
    /// Semver of the extension itself.
    pub version: String,
    /// Semver range of the host contract, e.g. `">=0.1, <0.2"`.
    pub carbon_api: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Package path of its icon (`ICON_TYPES`, at most `MAX_ICON_BYTES`;
    /// square, 128 px or SVG); the GUI shows a placeholder without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// From `CATEGORIES`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    /// What it may do (`capability.rs`); granted by the user at install.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// Other extensions it needs; never installed without the user.
    #[serde(default, skip_serializing_if = "Requires::is_empty")]
    pub requires: Requires,
    /// The extension's code: a WebAssembly component (package path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grammars: Vec<GrammarDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub language_servers: Vec<LanguageServerDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<ThemeDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<SettingDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menus: Vec<MenuDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panels: Vec<PanelDef>,
    /// Agent programs spoken to over ACP (API 0.2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<AgentDef>,
    /// Graph kinds for `graph` panels (API 0.2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphs: Vec<GraphSpec>,
    /// Node types of those graphs (API 0.2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_types: Vec<NodeTypeSpec>,
    /// Declarative views by name, for `view` panels (`view.rs`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub views: BTreeMap<String, Vec<Widget>>,
    /// Every other top-level key, warned about.
    #[serde(flatten)]
    other: toml::Table,
}

/// A language: which files it covers and how to colour them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageDef {
    /// `rust`, `typescript`, ...; language servers and the status bar use it.
    pub id: String,
    pub name: String,
    /// File name endings without the dot (`rs`, `d.ts`), matched case-insensitively.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suffixes: Vec<String>,
    /// Whole file names (`Makefile`, `Cargo.lock`), matched exactly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_names: Vec<String>,
    /// A grammar name (this or another extension's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_comment: Option<String>,
}

/// A tree-sitter grammar compiled to WebAssembly, with its queries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrammarDef {
    /// The tree-sitter language name (`rust`): the `.wasm` exports
    /// `tree_sitter_<name>`; injections name grammars by it.
    pub name: String,
    pub wasm: String,
    pub highlights: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injections: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locals: Option<String>,
}

/// A language server: the program and how to get it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageServerDef {
    /// Unique within the extension.
    pub id: String,
    pub name: String,
    /// Language ids it serves.
    pub languages: Vec<String>,
    /// Program name, looked up on PATH (`.exe` added on Windows).
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Shown when the program is not found and cannot be downloaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_hint: Option<String>,
    /// Builds to download when the program is not on PATH, by `TARGETS` name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub download: BTreeMap<String, Download>,
}

/// One downloadable build of a tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Download {
    /// `https://` only.
    pub url: String,
    /// SHA-256 of the downloaded file, 64 lowercase hex digits.
    pub sha256: String,
    /// The tool's version (names its folder; shown in the prompt).
    pub version: String,
    #[serde(default)]
    pub archive: Archive,
    /// Path of the program inside the archive; for `none` and `gz`, the
    /// name to save it as (default: `command`, `.exe` on Windows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Archive {
    /// The file is the program.
    #[default]
    None,
    /// The program, gzip-compressed.
    Gz,
    Zip,
    TarGz,
}

/// A theme file in the kit's `ThemeSet` JSON format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeDef {
    pub file: String,
}

/// A setting, registered as `<extension id>.<key>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingDef {
    /// Words separated by `.` or `_` (`check.command`).
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub kind: SettingKind,
    pub default: toml::Value,
    /// `choice`: the options as `{ value, label }`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<SettingChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// Category path under the extension's name (default: the name only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub user_only: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub restart: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingKind {
    Bool,
    Choice,
    Integer,
    Number,
    Text,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingChoice {
    pub value: String,
    pub label: String,
}

/// A command for the palette and menus. `action` runs an existing Carbon
/// action, `panel` opens one of this extension's panels; with neither, the
/// extension's code (`wasm`) runs it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDef {
    /// Unique within the extension; menus and views refer to it.
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// Host action name, e.g. `carbon::NewFile`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Arguments for actions that take them (as the keymap would pass them).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<toml::Table>,
    /// A `panels` id of this extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<String>,
}

/// A command placed at the end of a top-level menu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuDef {
    /// One of `MENUS`.
    pub menu: String,
    /// A `commands` id.
    pub command: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelKind {
    /// A declarative view drawn by Carbon (`view`).
    View,
    /// A web page from the package (`page`).
    Webview,
    /// A node-and-arrow graph drawn by Carbon (`graph`, API 0.2).
    Graph,
}

/// A panel the extension adds (View > Add Panel, its commands, layouts).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelDef {
    /// Unique within the extension.
    pub id: String,
    pub title: String,
    pub kind: PanelKind,
    /// `view` panels: a name in `views`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    /// `webview` panels: the page (package path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// `graph` panels: a `[[graphs]]` id of this extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
    /// One of `PLACEMENTS`; default `right`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<String>,
    /// A tab beside the Explorer in the left dock (the Explorer section).
    #[serde(default, skip_serializing_if = "is_false")]
    pub explorer_section: bool,
    /// More than one may be open.
    #[serde(default, skip_serializing_if = "is_false")]
    pub multi: bool,
    /// `view` panels: the extension's code draws it (`render-view`), and
    /// redraws it after each of its commands; `views` may hold a first look.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dynamic: bool,
}

impl PanelDef {
    pub fn placement(&self) -> &str {
        if self.explorer_section {
            return "left";
        }
        self.placement.as_deref().unwrap_or("right")
    }
}

/// An agent program Carbon talks to over ACP (Agent Client Protocol).
/// Found like a language server: on PATH, else installed from npm into
/// Carbon's tools folder, else run with `npx` (when allowed).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDef {
    /// Unique within the extension.
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Program name, looked up on PATH.
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// No secrets: the user's own environment passes through.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Shown when the program cannot be found or started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm: Option<NpmDef>,
}

/// An npm package that provides an agent program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpmDef {
    pub package: String,
    /// Exact version.
    pub version: String,
    /// The package's `bin` entry name.
    pub bin: String,
    /// Run `npx -y <package>@<version>` when installing is not possible.
    #[serde(default, skip_serializing_if = "is_false")]
    pub npx_fallback: bool,
}

/// Problems with inspector fields: keys unique and well formed, choices for
/// `choice`, item fields only for `list`.
fn check_fields(who: &str, fields: &[FieldSpec], problems: &mut Vec<String>) {
    let mut keys = HashSet::new();
    for field in fields {
        let key_ok = !field.key.is_empty()
            && field.key.split('.').all(|part| {
                !part.is_empty()
                    && part.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
                    })
            });
        if !key_ok {
            problems.push(format!(
                "{who}: field key `{}` must be lowercase words separated by `.`",
                field.key
            ));
        }
        if !keys.insert(field.key.as_str()) {
            problems.push(format!("{who}: field `{}` is declared twice", field.key));
        }
        if field.kind == FieldKind::Choice && field.choices.is_empty() {
            problems.push(format!("{who}: field `{}` needs choices", field.key));
        }
        if field.kind != FieldKind::List && !field.item_fields.is_empty() {
            problems.push(format!(
                "{who}: field `{}`: item_fields are for list fields",
                field.key
            ));
        }
        check_fields(
            &format!("{who} field `{}`", field.key),
            &field.item_fields,
            problems,
        );
    }
}

fn is_false(value: &bool) -> bool {
    !value
}

/// `check.command`, `tab_size`: words of lowercase letters, digits, `-`, `_`.
fn is_setting_key(key: &str) -> bool {
    !key.is_empty()
        && key.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        })
}

/// The host of an `https://` URL, lowercase.
fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Whether `net:<scope>` grants `host`.
fn net_allows(scope: &str, host: &str) -> bool {
    scope == "*"
        || scope == host
        || scope
            .strip_prefix("*.")
            .is_some_and(|domain| host.ends_with(&format!(".{domain}")))
}

fn sha256_ok(hex: &str) -> bool {
    hex.len() == 64
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl ExtensionDef {
    /// Parses a manifest. Warnings name top-level keys this build does not know.
    pub fn parse(text: &str) -> Result<(Self, Vec<String>), String> {
        let def: Self = toml::from_str(strip_bom(text)).map_err(|err| err.to_string())?;
        let warnings = def
            .other
            .keys()
            .map(|key| format!("unknown key `{key}` ignored"))
            .collect();
        Ok((def, warnings))
    }

    /// Whether the user granted (or the manifest asks for) running `command`.
    pub fn may_run(&self, command: &str) -> bool {
        self.capabilities.iter().any(|cap| {
            cap.strip_prefix("process.exec:")
                .is_some_and(|scope| scope == "*" || scope == command)
        })
    }

    /// Whether the manifest asks to reach `host`.
    pub fn may_reach(&self, host: &str) -> bool {
        self.capabilities.iter().any(|cap| {
            cap.strip_prefix("net:")
                .is_some_and(|scope| net_allows(scope, host))
        })
    }

    /// Every package path the manifest names (to check they exist).
    pub fn files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = Vec::new();
        files.extend(self.wasm.as_deref());
        files.extend(self.icon.as_deref());
        for grammar in &self.grammars {
            files.push(&grammar.wasm);
            files.push(&grammar.highlights);
            files.extend(grammar.injections.as_deref());
            files.extend(grammar.locals.as_deref());
        }
        files.extend(self.themes.iter().map(|theme| theme.file.as_str()));
        files.extend(self.panels.iter().filter_map(|panel| panel.page.as_deref()));
        files.extend(
            self.graphs
                .iter()
                .filter_map(|graph| graph.template.as_deref()),
        );
        files
    }

    /// Problems with the files the manifest names, inside package `dir`.
    pub fn check_files(&self, dir: &Path) -> Vec<String> {
        self.files()
            .into_iter()
            .filter(|path| check_package_path(path).is_ok())
            .filter_map(|path| resolve_package_file(dir, path).err())
            .collect()
    }

    /// Problems that keep this extension from loading; empty when valid.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.schema_version != SCHEMA_VERSION {
            problems.push(format!(
                "schema_version {} is not supported (this Carbon reads {SCHEMA_VERSION})",
                self.schema_version
            ));
        }
        if let Err(problem) = check_id(self.id.as_str()) {
            problems.push(problem);
        }
        if self.name.trim().is_empty() {
            problems.push("name is empty".into());
        }
        if Version::parse(&self.version).is_err() {
            problems.push(format!("version `{}` is not semver", self.version));
        }
        match VersionReq::parse(&self.carbon_api) {
            Ok(_) if self.api().is_none() => problems.push(format!(
                "needs Carbon API {}, this Carbon has {}",
                self.carbon_api,
                SUPPORTED_APIS.map(|api| api.to_string()).join(", ")
            )),
            Ok(_) => {}
            Err(_) => problems.push(format!(
                "carbon_api `{}` is not a version range",
                self.carbon_api
            )),
        }
        for category in &self.categories {
            if !CATEGORIES.contains(&category.as_str()) {
                problems.push(format!(
                    "category `{category}` is not one of {}",
                    CATEGORIES.join(", ")
                ));
            }
        }
        for capability in &self.capabilities {
            if let Err(problem) = check_capability(capability) {
                problems.push(problem);
            }
        }
        for path in self.files() {
            if let Err(problem) = check_package_path(path) {
                problems.push(problem);
            }
        }
        if let Some(Err(problem)) = self.icon.as_deref().map(check_icon_type) {
            problems.push(problem);
        }
        self.validate_languages(&mut problems);
        self.validate_servers(&mut problems);
        self.validate_settings(&mut problems);
        let command_ids = self.validate_commands(&mut problems);
        self.validate_panels(&command_ids, &mut problems);
        self.validate_agents(&mut problems);
        self.validate_graphs(&mut problems);
        problems.extend(self.requires.check(self.id.as_str()));
        problems
    }

    /// The host contract this extension gets: the newest supported one its
    /// `carbon_api` range matches.
    pub fn api(&self) -> Option<Version> {
        let req = VersionReq::parse(&self.carbon_api).ok()?;
        SUPPORTED_APIS
            .iter()
            .rev()
            .find(|api| req.matches(api))
            .cloned()
    }

    /// Whether it uses what API 0.2 brought (it then needs that contract).
    fn uses_graphs_api(&self) -> bool {
        !self.agents.is_empty()
            || !self.graphs.is_empty()
            || !self.node_types.is_empty()
            || self
                .panels
                .iter()
                .any(|panel| panel.kind == PanelKind::Graph)
    }

    fn validate_agents(&self, problems: &mut Vec<String>) {
        let mut ids = HashSet::new();
        for agent in &self.agents {
            let who = format!("agent `{}`", agent.id);
            if !is_word(&agent.id) {
                problems.push(format!("{who}: id must use lowercase letters, digits, `-`"));
            }
            if !ids.insert(agent.id.as_str()) {
                problems.push(format!("{who} is declared twice"));
            }
            if agent.name.trim().is_empty() {
                problems.push(format!("{who} needs a name"));
            }
            if agent.command.trim().is_empty() {
                problems.push(format!("{who} needs a command"));
            } else if !self.may_run(&agent.command) {
                problems.push(format!(
                    "{who}: add `process.exec:{}` to capabilities",
                    agent.command
                ));
            }
            let Some(npm) = &agent.npm else {
                continue;
            };
            if npm.package.trim().is_empty() || npm.bin.trim().is_empty() {
                problems.push(format!("{who}: npm needs `package` and `bin`"));
            }
            if Version::parse(&npm.version).is_err() {
                problems.push(format!(
                    "{who}: npm version `{}` must be an exact version",
                    npm.version
                ));
            }
            let mut needs = vec!["node", "npm"];
            if npm.npx_fallback {
                needs.push("npx");
            }
            for program in needs {
                if !self.may_run(program) {
                    problems.push(format!(
                        "{who}: add `process.exec:{program}` to capabilities"
                    ));
                }
            }
            if !self.may_reach("registry.npmjs.org") {
                problems.push(format!(
                    "{who}: add `net:registry.npmjs.org` to capabilities"
                ));
            }
        }
    }

    fn validate_graphs(&self, problems: &mut Vec<String>) {
        if self.uses_graphs_api() && self.api().is_some_and(|api| api < GRAPHS_API) {
            problems.push(format!(
                "agents, graphs, node types and graph panels need carbon_api >= {GRAPHS_API}"
            ));
        }
        let mut graph_ids = HashSet::new();
        for graph in &self.graphs {
            let who = format!("graph `{}`", graph.id);
            if let Err(problem) = check_graph_id(&graph.id) {
                problems.push(format!("{who}: {problem}"));
            }
            if !graph_ids.insert(graph.id.as_str()) {
                problems.push(format!("{who} is declared twice"));
            }
            if graph.name.trim().is_empty() {
                problems.push(format!("{who} needs a name"));
            }
            if let Err(problem) = check_package_path(&graph.dir) {
                problems.push(format!("{who}: dir {problem}"));
            }
            let suffix_ok = !graph.suffix.is_empty()
                && !graph.suffix.starts_with('.')
                && graph
                    .suffix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
            if !suffix_ok {
                problems.push(format!(
                    "{who}: suffix `{}` must be letters, digits, `.` and `-`, without a leading dot",
                    graph.suffix
                ));
            }
            if graph.check && self.wasm.is_none() {
                problems.push(format!("{who}: `check` needs the extension's `wasm` code"));
            }
            let mut actions = HashSet::new();
            for action in &graph.actions {
                if !actions.insert(action.id.as_str()) {
                    problems.push(format!("{who}: action `{}` is declared twice", action.id));
                }
                if let Some(when) = &action.when
                    && !["idle", "running"].contains(&when.as_str())
                {
                    problems.push(format!(
                        "{who}: action `{}`: when must be `idle` or `running`",
                        action.id
                    ));
                }
            }
            if !graph.actions.is_empty() && self.wasm.is_none() {
                problems.push(format!("{who}: actions need the extension's `wasm` code"));
            }
            check_fields(&who, &graph.fields, problems);
        }
        let mut type_ids = HashSet::new();
        for spec in &self.node_types {
            let who = format!("node type `{}`", spec.id);
            if !graph_ids.contains(spec.graph.as_str()) {
                problems.push(format!("{who}: no graph `{}` in [[graphs]]", spec.graph));
            }
            if let Err(problem) = check_graph_id(&spec.id) {
                problems.push(format!("{who}: {problem}"));
            }
            if !type_ids.insert((spec.graph.as_str(), spec.id.as_str())) {
                problems.push(format!("{who} is declared twice"));
            }
            if spec.title.trim().is_empty() {
                problems.push(format!("{who} needs a title"));
            }
            let mut ports = HashSet::new();
            for port in spec.inputs.iter().chain(&spec.outputs) {
                if let Err(problem) = check_graph_id(&port.id) {
                    problems.push(format!("{who}: port {problem}"));
                }
                if !ports.insert(port.id.as_str()) {
                    problems.push(format!("{who}: port `{}` is declared twice", port.id));
                }
            }
            if spec.outputs.iter().any(|port| port.loop_back) {
                problems.push(format!("{who}: only inputs can be `loop_back`"));
            }
            if let Some(key) = &spec.outputs_from {
                let list = spec
                    .fields
                    .iter()
                    .any(|field| &field.key == key && field.kind == FieldKind::List);
                if !list {
                    problems.push(format!(
                        "{who}: outputs_from `{key}` must name a `list` field"
                    ));
                }
            }
            check_fields(&who, &spec.fields, problems);
        }
        for panel in &self.panels {
            let who = format!("panel `{}`", panel.id);
            match (&panel.kind, &panel.graph) {
                (PanelKind::Graph, None) => {
                    problems.push(format!("{who}: a graph panel needs `graph`"))
                }
                (PanelKind::Graph, Some(graph)) if !graph_ids.contains(graph.as_str()) => {
                    problems.push(format!("{who}: no graph `{graph}` in [[graphs]]"))
                }
                (PanelKind::View | PanelKind::Webview, Some(_)) => {
                    problems.push(format!("{who}: `graph` is only for graph panels"))
                }
                _ => {}
            }
        }
    }

    fn validate_languages(&self, problems: &mut Vec<String>) {
        let mut ids = HashSet::new();
        for lang in &self.languages {
            if !is_word(&lang.id) {
                problems.push(format!(
                    "language id `{}` must use lowercase letters, digits, `-`",
                    lang.id
                ));
            }
            if !ids.insert(lang.id.as_str()) {
                problems.push(format!("language `{}` is declared twice", lang.id));
            }
            if lang.name.trim().is_empty() {
                problems.push(format!("language `{}` needs a name", lang.id));
            }
            if lang.suffixes.is_empty() && lang.file_names.is_empty() {
                problems.push(format!(
                    "language `{}` needs `suffixes` or `file_names`",
                    lang.id
                ));
            }
            for suffix in &lang.suffixes {
                if suffix.is_empty() || suffix.starts_with('.') || suffix.contains(['/', '\\']) {
                    problems.push(format!(
                        "language `{}`: suffix `{suffix}` must be a file ending without the dot",
                        lang.id
                    ));
                }
            }
        }
        let mut names = HashSet::new();
        for grammar in &self.grammars {
            let ok = !grammar.name.is_empty()
                && grammar
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !ok {
                problems.push(format!(
                    "grammar `{}` must use lowercase letters, digits, `_`",
                    grammar.name
                ));
            }
            if !names.insert(grammar.name.as_str()) {
                problems.push(format!("grammar `{}` is declared twice", grammar.name));
            }
        }
    }

    fn validate_servers(&self, problems: &mut Vec<String>) {
        let languages: HashSet<&str> = self.languages.iter().map(|l| l.id.as_str()).collect();
        let mut ids = HashSet::new();
        for server in &self.language_servers {
            let who = format!("language server `{}`", server.id);
            if !is_word(&server.id) {
                problems.push(format!("{who}: id must use lowercase letters, digits, `-`"));
            }
            if !ids.insert(server.id.as_str()) {
                problems.push(format!("{who} is declared twice"));
            }
            if server.languages.is_empty() {
                problems.push(format!("{who} serves no languages"));
            }
            for language in &server.languages {
                // Serving another extension's language is fine; a typo in an
                // extension that declares its own languages is not.
                if !is_word(language) {
                    problems.push(format!("{who}: `{language}` is not a language id"));
                } else if !languages.is_empty() && !languages.contains(language.as_str()) {
                    problems.push(format!(
                        "{who}: language `{language}` is not in this extension's [[languages]]"
                    ));
                }
            }
            let plain = !server.command.is_empty()
                && server
                    .command
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
            if !plain {
                problems.push(format!(
                    "{who}: command `{}` must be a program name (no path)",
                    server.command
                ));
            } else if !self.may_run(&server.command) {
                problems.push(format!(
                    "{who}: add `process.exec:{}` to capabilities",
                    server.command
                ));
            }
            for (target, download) in &server.download {
                if !TARGETS.contains(&target.as_str()) {
                    problems.push(format!(
                        "{who}: download target `{target}` is not one of {}",
                        TARGETS.join(", ")
                    ));
                }
                match https_host(&download.url) {
                    None => problems.push(format!(
                        "{who}: download url `{}` must start with https://",
                        download.url
                    )),
                    Some(host) if !self.may_reach(&host) => problems.push(format!(
                        "{who}: add `net:{host}` to capabilities to download from it"
                    )),
                    Some(_) => {}
                }
                if !sha256_ok(&download.sha256) {
                    problems.push(format!(
                        "{who}: sha256 must be 64 lowercase hex digits ({target})"
                    ));
                }
                if check_package_path(&download.version).is_err() || download.version.contains('/')
                {
                    problems.push(format!(
                        "{who}: download version `{}` must be a plain folder name ({target})",
                        download.version
                    ));
                }
                if let Some(bin) = &download.bin
                    && let Err(problem) = check_package_path(bin)
                {
                    problems.push(format!("{who}: bin {problem}"));
                }
            }
        }
    }

    fn validate_settings(&self, problems: &mut Vec<String>) {
        let mut keys = HashSet::new();
        for setting in &self.settings {
            if !is_setting_key(&setting.key) {
                problems.push(format!(
                    "setting `{}`: key must be lowercase words separated by `.`",
                    setting.key
                ));
            }
            if !keys.insert(setting.key.as_str()) {
                problems.push(format!("setting `{}` is declared twice", setting.key));
            }
            if let Err(problem) = setting.to_setting(self) {
                problems.push(problem);
            }
        }
    }

    fn validate_commands<'a>(&'a self, problems: &mut Vec<String>) -> HashSet<&'a str> {
        let panels: HashSet<&str> = self.panels.iter().map(|p| p.id.as_str()).collect();
        let mut ids = HashSet::new();
        for command in &self.commands {
            let who = format!("command `{}`", command.id);
            if !is_word(&command.id) {
                problems.push(format!("{who}: id must use lowercase letters, digits, `-`"));
            }
            if !ids.insert(command.id.as_str()) {
                problems.push(format!("{who} is declared twice"));
            }
            if command.title.trim().is_empty() {
                problems.push(format!("{who} needs a title"));
            }
            match (&command.action, &command.panel) {
                (Some(_), Some(_)) => {
                    problems.push(format!("{who}: use `action` or `panel`, not both"))
                }
                (None, Some(panel)) if !panels.contains(panel.as_str()) => {
                    problems.push(format!("{who}: no panel `{panel}` in [[panels]]"))
                }
                (None, None) if self.wasm.is_none() => problems.push(format!(
                    "{who}: needs `action`, `panel`, or the extension's `wasm` code"
                )),
                _ => {}
            }
        }
        for item in &self.menus {
            if !MENUS.contains(&item.menu.as_str()) {
                problems.push(format!(
                    "menu `{}` is not one of {}",
                    item.menu,
                    MENUS.join(", ")
                ));
            }
            if !ids.contains(item.command.as_str()) {
                problems.push(format!("menu item: no command `{}`", item.command));
            }
        }
        ids
    }

    fn validate_panels(&self, command_ids: &HashSet<&str>, problems: &mut Vec<String>) {
        let mut ids = HashSet::new();
        for panel in &self.panels {
            let who = format!("panel `{}`", panel.id);
            if !is_word(&panel.id) {
                problems.push(format!("{who}: id must use lowercase letters, digits, `-`"));
            }
            if !ids.insert(panel.id.as_str()) {
                problems.push(format!("{who} is declared twice"));
            }
            if panel.title.trim().is_empty() {
                problems.push(format!("{who} needs a title"));
            }
            if let Some(placement) = &panel.placement
                && !PLACEMENTS.contains(&placement.as_str())
            {
                problems.push(format!(
                    "{who}: placement `{placement}` is not one of {}",
                    PLACEMENTS.join(", ")
                ));
            }
            if panel.dynamic && (panel.kind != PanelKind::View || self.wasm.is_none()) {
                problems.push(format!(
                    "{who}: `dynamic` is for view panels of an extension with `wasm` code"
                ));
            }
            match panel.kind {
                PanelKind::View => match &panel.view {
                    None => problems.push(format!("{who}: a view panel needs `view`")),
                    Some(view) if !panel.dynamic && !self.views.contains_key(view) => {
                        problems.push(format!("{who}: no view `{view}` in [views]"))
                    }
                    Some(_) => {}
                },
                PanelKind::Webview => {
                    if panel.page.is_none() {
                        problems.push(format!("{who}: a webview panel needs `page`"));
                    }
                }
                // Checked with the graphs (`validate_graphs`).
                PanelKind::Graph => {}
            }
            if panel.kind != PanelKind::Webview && panel.page.is_some() {
                problems.push(format!("{who}: `page` is only for webview panels"));
            }
            if panel.kind != PanelKind::View && panel.view.is_some() {
                problems.push(format!("{who}: `view` is only for view panels"));
            }
        }
        for (name, widgets) in &self.views {
            problems.extend(view::check_view(name, widgets, command_ids));
        }
    }
}

/// A TOML value as JSON (settings files are JSON).
fn to_json(value: &toml::Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

impl SettingDef {
    /// The settings registry's declaration, keyed `<extension id>.<key>`.
    pub fn to_setting(&self, ext: &ExtensionDef) -> Result<Setting, String> {
        let who = format!("setting `{}`", self.key);
        let kind = match self.kind {
            SettingKind::Bool => Kind::Bool,
            SettingKind::Text => Kind::Text,
            SettingKind::Choice => {
                if self.choices.is_empty() {
                    return Err(format!("{who}: a choice needs `choices`"));
                }
                Kind::Choice(
                    self.choices
                        .iter()
                        .map(|choice| Choice {
                            value: choice.value.clone(),
                            label: choice.label.clone(),
                        })
                        .collect(),
                )
            }
            SettingKind::Integer => Kind::integer(
                self.min.map_or(i64::MIN, |n| n as i64),
                self.max.map_or(i64::MAX, |n| n as i64),
            ),
            SettingKind::Number => Kind::number(
                self.min.unwrap_or(f64::MIN),
                self.max.unwrap_or(f64::MAX),
                self.step.unwrap_or(1.0),
            ),
        };
        let default = to_json(&self.default);
        if !kind.accepts(&default) {
            return Err(format!("{who}: the default does not fit its kind"));
        }
        let mut category = vec![ext.name.clone()];
        category.extend(self.category.iter().cloned());
        let category: Vec<&str> = category.iter().map(String::as_str).collect();
        let mut setting = Setting::new(
            &format!("{}.{}", ext.id, self.key),
            &category,
            &self.label,
            kind,
            default,
        )
        .description(&self.description);
        setting.user_only = self.user_only;
        setting.restart = self.restart;
        setting.extension = Some(ext.id.to_string());
        Ok(setting)
    }
}
