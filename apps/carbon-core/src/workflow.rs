//! Workflows: switchable app modes, one Section Bar icon each, described by a
//! `workflow.toml` manifest (`project/not_built/workflow-system.md`).
//!
//! A manifest names its default layout in panel *kinds* (`editor`, `chat`,
//! ...); the GUI turns kinds into panels. Layers: Carbon's own workflows as
//! shipped package folders (`apps/carbon-workflow/<name>/`, read-only),
//! installed packages, user workflows, and overlays that patch a shipped or
//! installed one (`overlay.rs`). Saved layouts and flags live in `state.rs`;
//! folders in `dirs.rs`. Plain data with no UI types.

mod dirs;
mod overlay;
mod package;
mod state;
mod view;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

pub use dirs::{SHIPPED_ENV, WorkflowDirs, shipped_dir};
pub use overlay::{Overlay, RegionPatch};
pub use package::{MAX_PACKAGE_PATH, check_package_path, resolve_package_file};
pub use state::{SavedWorkflow, StateFile, finish_entering, start_entering, take_unfinished};
pub use view::{Align, ButtonStyle, ListItem, MAX_VIEW_DEPTH, MAX_VIEW_WIDGETS, Widget};

/// Manifest format version this build reads.
pub const SCHEMA_VERSION: u32 = 1;
/// Host contract a manifest's `carbon_api` range is checked against.
pub const API_VERSION: Version = Version::new(0, 1, 0);
/// Id of the Editor workflow, the default.
pub const EDITOR: &str = "carbon.editor";
/// Ids under this prefix belong to built-in workflows.
const BUILTIN_PREFIX: &str = "carbon.";
/// Panel kind with no content of its own: shows its `title`.
pub const PLACEHOLDER_KIND: &str = "placeholder";
/// Panel kind showing a web page from the workflow's package (`page`).
pub const WEBVIEW_KIND: &str = "webview";
/// Panel kind drawing one of the manifest's declarative `views` (`view`).
pub const VIEW_KIND: &str = "view";
/// Top-level menus a workflow may add items to.
pub const MENUS: [&str; 4] = ["File", "Edit", "View", "Help"];
/// Layout regions, as named in manifests and overlays.
pub const REGIONS: [&str; 4] = ["center", "left", "right", "bottom"];

/// The Editor's shipped manifest, also compiled in: Carbon still opens when
/// the shipped folder is missing or moved.
const EDITOR_FALLBACK: &str = include_str!("../../carbon-workflow/editor/workflow.toml");
/// Source name of the compiled-in Editor in warnings and `origin`.
const EDITOR_FALLBACK_SOURCE: &str = "built-in Editor";

/// Manifest keys from the design that this build accepts but does not use yet.
const RESERVED_KEYS: [&str; 9] = [
    "derived_from",
    "keybindings",
    "activation",
    "file_types",
    "settings",
    "background",
    "requires",
    "capabilities",
    "agent",
];

/// A workflow's stable id (`publisher.name`); it keys saved layout state.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowId(String);

impl WorkflowId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn editor() -> Self {
        Self::new(EDITOR)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Under `carbon.*`: only built-ins may use it.
    pub fn is_builtin(&self) -> bool {
        self.0.starts_with(BUILTIN_PREFIX)
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One `workflow.toml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowDef {
    pub id: WorkflowId,
    pub schema_version: u32,
    /// Semver of the workflow itself.
    pub version: String,
    /// Semver range of the host contract, e.g. `">=0.1, <0.2"`.
    pub carbon_api: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Lucide icon name (`code`, `bot`, ...); the GUI falls back to a generic one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Section Bar position, ascending.
    #[serde(default = "default_order")]
    pub order: i32,
    /// Not shown in the Section Bar or View > Section.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// Keys this workflow had in saved files before it became a workflow.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legacy_ids: Vec<String>,
    /// Shows an "open a folder" state without one (not used yet).
    #[serde(default, skip_serializing_if = "is_false")]
    pub requires_workspace: bool,
    #[serde(default)]
    pub layout: Layout,
    /// Panels by reference name; layout regions list these names.
    #[serde(default)]
    pub panels: BTreeMap<String, PanelRule>,
    /// Commands shown in the palette while this workflow is active.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandDef>,
    /// Places for those commands in the top-level menus.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menus: Vec<MenuDef>,
    /// Declarative views by name, for `view` panels (`view.rs`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub views: BTreeMap<String, Vec<Widget>>,
    /// Every other top-level key: reserved ones are ignored, the rest warned about.
    #[serde(flatten)]
    other: toml::Table,
}

fn default_order() -> i32 {
    1000
}

fn is_false(value: &bool) -> bool {
    !value
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Default layout: the center and the three docks. A missing dock starts empty and closed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<Region>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<Region>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<Region>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Region>,
}

impl Layout {
    /// A region by name (`REGIONS`).
    pub fn region_mut(&mut self, name: &str) -> Option<&mut Option<Region>> {
        match name {
            "center" => Some(&mut self.center),
            "left" => Some(&mut self.left),
            "right" => Some(&mut self.right),
            "bottom" => Some(&mut self.bottom),
            _ => None,
        }
    }
}

/// One region of the layout: its panels as tabs, in order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// Names from the manifest's `[panels]` table.
    #[serde(default)]
    pub panels: Vec<String>,
    /// Width (left/right) or height (bottom) in pixels; the host default if unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub open: bool,
}

/// A panel the workflow uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelRule {
    /// Panel kind; defaults to the reference name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Tab title; required for placeholders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// More than one may be open (not enforced yet).
    #[serde(default, skip_serializing_if = "is_false")]
    pub multi: bool,
    /// The user may close it (not enforced yet).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub closable: bool,
    /// Webview panels: the page, a path inside the workflow's package
    /// (`check_package_path` rules).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// View panels: a name in the manifest's `views`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
}

fn yes() -> bool {
    true
}

/// A workflow command: runs an existing host action, so it cannot add behaviour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDef {
    /// Unique within the workflow; `menus` refer to it.
    pub id: String,
    /// Palette and menu text.
    pub label: String,
    /// Host action name, e.g. `carbon::NewFile`.
    pub run: String,
    /// Arguments for actions that take them (as the keymap would pass them).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<toml::Table>,
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

impl WorkflowDef {
    /// Parses a manifest. Warnings name top-level keys this build does not know.
    pub fn parse(text: &str) -> Result<(Self, Vec<String>), String> {
        let def: Self = toml::from_str(strip_bom(text)).map_err(|err| err.to_string())?;
        let warnings = def
            .other
            .keys()
            .filter(|key| !RESERVED_KEYS.contains(&key.as_str()))
            .map(|key| format!("unknown key `{key}` ignored"))
            .collect();
        Ok((def, warnings))
    }

    /// The manifest as TOML (what Duplicate writes).
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string(self).map_err(|err| err.to_string())
    }

    /// A copy under a new id and name, for the user to change: no legacy
    /// ids, shown, placed right after this one, and `derived_from` naming
    /// this workflow and its version.
    pub fn duplicate(&self, id: WorkflowId, name: impl Into<String>) -> Self {
        let mut copy = self.clone();
        copy.id = id;
        copy.name = name.into();
        copy.legacy_ids.clear();
        copy.hidden = false;
        copy.order = self.order.saturating_add(1);
        let mut parent = toml::Table::new();
        parent.insert("id".into(), self.id.to_string().into());
        parent.insert("version".into(), self.version.clone().into());
        copy.other.insert("derived_from".into(), parent.into());
        copy
    }

    /// The kind of a panel reference (its rule's `kind`, else the name itself).
    pub fn panel_kind<'a>(&'a self, name: &'a str) -> &'a str {
        self.panels
            .get(name)
            .and_then(|rule| rule.kind.as_deref())
            .unwrap_or(name)
    }

    /// Regions as (name, region) for those the manifest sets.
    pub fn regions(&self) -> impl Iterator<Item = (&'static str, &Region)> {
        [
            ("center", &self.layout.center),
            ("left", &self.layout.left),
            ("right", &self.layout.right),
            ("bottom", &self.layout.bottom),
        ]
        .into_iter()
        .filter_map(|(name, region)| region.as_ref().map(|region| (name, region)))
    }

    /// `region:kind` for every panel of the default layout, in order. Saved
    /// with a layout so a later change of the default can be detected.
    pub fn base_panels(&self) -> Vec<String> {
        self.regions()
            .flat_map(|(region_name, region)| {
                region
                    .panels
                    .iter()
                    .map(move |name| format!("{region_name}:{}", self.panel_kind(name)))
            })
            .collect()
    }

    /// Hash of the default layout and panel rules (FNV-1a, hex). Changes when
    /// the default layout changes, not when the name, icon or order do.
    pub fn layout_fingerprint(&self) -> String {
        let text = format!("{:?}{:?}", self.layout, self.panels);
        let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
        format!("{hash:016x}")
    }

    /// Panel kinds the layout uses that this host cannot build; those panels
    /// show as "not available" placeholders.
    pub fn unavailable_kinds(&self, panel_kinds: &[&str]) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .panels
            .keys()
            .map(|name| self.panel_kind(name))
            .filter(|kind| !panel_kinds.contains(kind))
            .map(str::to_string)
            .collect();
        kinds.dedup();
        kinds
    }

    /// Problems that keep this workflow from loading; empty when valid.
    /// `builtin`: may use `carbon.*`.
    pub fn validate(&self, builtin: bool) -> Vec<String> {
        let mut problems = Vec::new();
        if self.schema_version != SCHEMA_VERSION {
            problems.push(format!(
                "schema_version {} is not supported (this Carbon reads {SCHEMA_VERSION})",
                self.schema_version
            ));
        }
        if let Err(problem) = check_id(self.id.as_str()) {
            problems.push(problem);
        } else if self.id.is_builtin() && !builtin {
            problems.push(format!("id `{}`: `carbon.*` is reserved", self.id));
        }
        if self.name.trim().is_empty() {
            problems.push("name is empty".into());
        }
        if Version::parse(&self.version).is_err() {
            problems.push(format!("version `{}` is not semver", self.version));
        }
        match VersionReq::parse(&self.carbon_api) {
            Ok(req) if !req.matches(&API_VERSION) => problems.push(format!(
                "needs Carbon API {}, this Carbon has {API_VERSION}",
                self.carbon_api
            )),
            Ok(_) => {}
            Err(_) => problems.push(format!(
                "carbon_api `{}` is not a version range",
                self.carbon_api
            )),
        }
        for legacy in &self.legacy_ids {
            if legacy.is_empty() || legacy.contains('.') {
                problems.push(format!("legacy id `{legacy}` must be a plain word"));
            }
        }
        if self
            .layout
            .center
            .as_ref()
            .is_none_or(|center| center.panels.is_empty())
        {
            problems.push("layout.center needs at least one panel".into());
        }
        for (region_name, region) in self.regions() {
            if region.size.is_some_and(|size| size.is_nan() || size <= 0.0) {
                problems.push(format!("layout.{region_name}.size must be positive"));
            }
            for name in &region.panels {
                if !self.panels.contains_key(name) {
                    problems.push(format!(
                        "layout.{region_name}: panel `{name}` is not in [panels]"
                    ));
                }
            }
        }
        for (name, rule) in &self.panels {
            if self.panel_kind(name) == PLACEHOLDER_KIND
                && rule.title.as_deref().is_none_or(str::is_empty)
            {
                problems.push(format!("panel `{name}`: a placeholder needs a title"));
            }
            if let Some(page) = &rule.page {
                if self.panel_kind(name) != WEBVIEW_KIND {
                    problems.push(format!(
                        "panel `{name}`: `page` is only for `{WEBVIEW_KIND}` panels"
                    ));
                } else if let Err(problem) = check_package_path(page) {
                    problems.push(format!("panel `{name}`: page {problem}"));
                }
            }
            match (&rule.view, self.panel_kind(name) == VIEW_KIND) {
                (Some(_), false) => problems.push(format!(
                    "panel `{name}`: `view` is only for `{VIEW_KIND}` panels"
                )),
                (None, true) => problems.push(format!("panel `{name}`: a view panel needs `view`")),
                (Some(view), true) if !self.views.contains_key(view) => {
                    problems.push(format!("panel `{name}`: no view `{view}` in [views]"))
                }
                _ => {}
            }
        }
        let mut command_ids = HashSet::new();
        for command in &self.commands {
            if !is_word(&command.id) {
                problems.push(format!(
                    "command id `{}` must use lowercase letters, digits, `-`",
                    command.id
                ));
            }
            if !command_ids.insert(command.id.as_str()) {
                problems.push(format!("command id `{}` is used twice", command.id));
            }
            if command.label.trim().is_empty() || command.run.trim().is_empty() {
                problems.push(format!("command `{}` needs a label and run", command.id));
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
            if !command_ids.contains(item.command.as_str()) {
                problems.push(format!("menu item: no command `{}`", item.command));
            }
        }
        for (name, widgets) in &self.views {
            problems.extend(view::check_view(name, widgets, &command_ids));
        }
        problems
    }
}

/// Text without a leading UTF-8 byte order mark (some Windows editors write one).
fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
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

/// Where a workflow came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Carbon's own: a shipped package folder (read-only), or the compiled-in
    /// Editor. Changed only through an overlay; never removed.
    Builtin,
    /// A downloaded package in the installed folder (Phase 4 installs them).
    Installed(PathBuf),
    /// A user `workflow.toml` at this path.
    User(PathBuf),
}

/// A manifest or overlay that did not load, with why.
#[derive(Clone, Debug)]
pub struct Rejected {
    /// File name or path of the manifest or overlay.
    pub source: String,
    pub problems: Vec<String>,
}

/// The loaded workflows, sorted by `order` then id.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    workflows: Vec<WorkflowDef>,
    origins: HashMap<WorkflowId, Origin>,
    /// Overlay file applied to a workflow.
    overlays: HashMap<WorkflowId, String>,
    /// Disabled or quarantined: kept but not offered.
    blocked: HashSet<WorkflowId>,
    rejected: Vec<Rejected>,
    warnings: Vec<(String, String)>,
    /// Ids and legacy ids taken, by source.
    keys: HashMap<String, String>,
    /// Package folder of each workflow loaded from one (pages and assets
    /// resolve inside it); none for the compiled-in Editor.
    packages: HashMap<WorkflowId, PathBuf>,
}

impl Registry {
    /// Carbon's own workflows from the shipped folder (`shipped_dir`).
    pub fn builtin(panel_kinds: &[&str]) -> Self {
        Self::shipped(shipped_dir().as_deref(), panel_kinds)
    }

    /// Carbon's own workflows from `<dir>/<name>/workflow.toml`. The Editor is
    /// added from the compiled-in copy when the folder lacks it.
    pub fn shipped(dir: Option<&std::path::Path>, panel_kinds: &[&str]) -> Self {
        let mut registry = Self::default();
        for (path, text) in dir.map(dirs::package_manifests).unwrap_or_default() {
            let source = path.display().to_string();
            match text {
                Ok(text) => {
                    if let Some(id) = registry.add(&source, &text, panel_kinds, Origin::Builtin)
                        && let Some(dir) = path.parent()
                    {
                        registry.packages.insert(id, dir.to_path_buf());
                    }
                }
                Err(err) => registry.reject(&source, vec![err]),
            }
        }
        if registry.get(&WorkflowId::editor()).is_none() {
            let source = EDITOR_FALLBACK_SOURCE;
            registry.add(source, EDITOR_FALLBACK, panel_kinds, Origin::Builtin);
            let place = dir.map_or("(none)".into(), |dir| dir.display().to_string());
            registry.warn(
                source,
                vec![format!(
                    "shipped workflows not found in {place}; using the built-in Editor"
                )],
            );
        }
        registry.sort();
        registry
    }

    /// Carbon's own workflows, then (with `dirs`) installed packages, user
    /// workflows and overlays. Without `dirs` (safe mode) only Carbon's own.
    pub fn load(dirs: Option<&WorkflowDirs>, panel_kinds: &[&str]) -> Self {
        let mut registry = Self::builtin(panel_kinds);
        let Some(dirs) = dirs else {
            return registry;
        };
        for (path, text) in dirs.installed_manifests() {
            let source = path.display().to_string();
            match text {
                Ok(text) => {
                    registry.add(&source, &text, panel_kinds, Origin::Installed(path));
                }
                Err(err) => registry.reject(&source, vec![err]),
            }
        }
        for (path, text) in dirs.user_manifests() {
            let source = path.display().to_string();
            match text {
                Ok(text) => {
                    registry.add(&source, &text, panel_kinds, Origin::User(path));
                }
                Err(err) => registry.reject(&source, vec![err]),
            }
        }
        for (path, id, text) in dirs.overlays() {
            let source = path.display().to_string();
            match text {
                Ok(text) => {
                    registry.apply_overlay(&source, &WorkflowId::new(id), &text, panel_kinds)
                }
                Err(err) => registry.reject(&source, vec![err]),
            }
        }
        registry.sort();
        registry
    }

    /// Loads manifests given as (file name, text). A bad one is rejected with
    /// its reasons and never stops the others; a duplicate id or legacy id
    /// rejects the later file.
    pub fn from_sources(sources: &[(&str, &str)], panel_kinds: &[&str], builtin: bool) -> Self {
        let mut registry = Self::default();
        for (source, text) in sources {
            let origin = if builtin {
                Origin::Builtin
            } else {
                Origin::User(PathBuf::from(source))
            };
            registry.add(source, text, panel_kinds, origin);
        }
        registry.sort();
        registry
    }

    /// Adds a manifest; its id when it loaded. A user or installed workflow's
    /// package folder is the manifest's folder.
    fn add(
        &mut self,
        source: &str,
        text: &str,
        panel_kinds: &[&str],
        origin: Origin,
    ) -> Option<WorkflowId> {
        let (def, warnings) = match WorkflowDef::parse(text) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.reject(source, vec![err]);
                return None;
            }
        };
        let mut problems = def.validate(origin == Origin::Builtin);
        for key in std::iter::once(def.id.as_str()).chain(def.legacy_ids.iter().map(String::as_str))
        {
            if let Some(owner) = self.keys.get(key) {
                problems.push(format!("id `{key}` is already used by {owner}"));
            }
        }
        if !problems.is_empty() {
            self.reject(source, problems);
            return None;
        }
        for key in std::iter::once(def.id.to_string()).chain(def.legacy_ids.iter().cloned()) {
            self.keys.insert(key, source.to_string());
        }
        self.warn(source, warnings);
        self.warn_unavailable(source, &def, panel_kinds);
        if let Origin::Installed(manifest) | Origin::User(manifest) = &origin
            && let Some(dir) = manifest.parent().filter(|dir| !dir.as_os_str().is_empty())
        {
            self.packages.insert(def.id.clone(), dir.to_path_buf());
        }
        let id = def.id.clone();
        self.origins.insert(def.id.clone(), origin);
        self.workflows.push(def);
        Some(id)
    }

    /// Patches a shipped or installed workflow with an overlay file. A bad
    /// overlay is rejected and the workflow keeps its base definition.
    pub fn apply_overlay(
        &mut self,
        source: &str,
        id: &WorkflowId,
        text: &str,
        panel_kinds: &[&str],
    ) {
        let Some(index) = self.workflows.iter().position(|def| &def.id == id) else {
            self.reject(source, vec![format!("no workflow `{id}` to change")]);
            return;
        };
        if matches!(self.origins.get(id), Some(Origin::User(_))) {
            self.reject(
                source,
                vec![format!(
                    "`{id}` is a user workflow: edit its workflow.toml instead"
                )],
            );
            return;
        }
        let overlay = match Overlay::parse(text) {
            Ok(overlay) => overlay,
            Err(err) => {
                self.reject(source, vec![err]);
                return;
            }
        };
        let (patched, notices) = match overlay.apply(&self.workflows[index]) {
            Ok(patched) => patched,
            Err(problems) => {
                self.reject(source, problems);
                return;
            }
        };
        let problems = patched.validate(self.origins.get(id) == Some(&Origin::Builtin));
        if !problems.is_empty() {
            self.reject(source, problems);
            return;
        }
        self.warn(source, notices);
        self.warn_unavailable(source, &patched, panel_kinds);
        self.workflows[index] = patched;
        self.overlays.insert(id.clone(), source.to_string());
        self.sort();
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

    fn warn_unavailable(&mut self, source: &str, def: &WorkflowDef, panel_kinds: &[&str]) {
        let warnings = def
            .unavailable_kinds(panel_kinds)
            .into_iter()
            .map(|kind| format!("panel kind `{kind}` is not available; shown as a placeholder"))
            .collect();
        self.warn(source, warnings);
    }

    fn sort(&mut self) {
        self.workflows
            .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
    }

    /// Keeps these workflows out of `available` (disabled or quarantined).
    /// The Editor is never blocked: it is the default and safe-mode workflow.
    pub fn block(&mut self, ids: impl IntoIterator<Item = WorkflowId>) {
        let editor = WorkflowId::editor();
        self.blocked
            .extend(ids.into_iter().filter(|id| *id != editor));
    }

    /// Its files can be deleted: user and installed workflows. Carbon's own
    /// packages are read-only; they can only be hidden or turned off.
    pub fn removable(&self, id: &WorkflowId) -> bool {
        matches!(
            self.origins.get(id),
            Some(Origin::User(_) | Origin::Installed(_))
        )
    }

    /// May be hidden or turned off: every workflow but the Editor.
    pub fn can_turn_off(&self, id: &WorkflowId) -> bool {
        *id != WorkflowId::editor() && self.get(id).is_some()
    }

    /// The shipped folder was missing or had no Editor, so the compiled-in
    /// Editor is used.
    pub fn uses_editor_fallback(&self) -> bool {
        self.warnings
            .iter()
            .any(|(source, _)| source == EDITOR_FALLBACK_SOURCE)
    }

    /// Every loaded workflow, including hidden and blocked ones.
    pub fn workflows(&self) -> &[WorkflowDef] {
        &self.workflows
    }

    /// The workflows the Section Bar and View > Section offer.
    pub fn available(&self) -> impl Iterator<Item = &WorkflowDef> {
        self.workflows
            .iter()
            .filter(|def| !def.hidden && !self.blocked.contains(&def.id))
    }

    pub fn is_available(&self, id: &WorkflowId) -> bool {
        self.available().any(|def| &def.id == id)
    }

    pub fn origin(&self, id: &WorkflowId) -> Option<&Origin> {
        self.origins.get(id)
    }

    /// The folder `id`'s package files resolve in (`resolve_package_file`).
    pub fn package_dir(&self, id: &WorkflowId) -> Option<&Path> {
        self.packages.get(id).map(PathBuf::as_path)
    }

    /// The overlay file applied to `id`, if any.
    pub fn overlay(&self, id: &WorkflowId) -> Option<&str> {
        self.overlays.get(id).map(String::as_str)
    }

    pub fn rejected(&self) -> &[Rejected] {
        &self.rejected
    }

    /// (source, warning): ignored keys, unavailable panel kinds, overlay notices.
    pub fn warnings(&self) -> &[(String, String)] {
        &self.warnings
    }

    pub fn get(&self, id: &WorkflowId) -> Option<&WorkflowDef> {
        self.workflows.iter().find(|def| &def.id == id)
    }

    /// A workflow by id or by one of its legacy ids.
    pub fn resolve(&self, key: &str) -> Option<&WorkflowDef> {
        self.workflows
            .iter()
            .find(|def| def.id.as_str() == key || def.legacy_ids.iter().any(|id| id == key))
    }

    /// A free id for a user copy of a workflow named `name`: `user.<slug>`,
    /// with `-2`, `-3`, ... when taken.
    pub fn free_user_id(&self, name: &str) -> WorkflowId {
        let mut slug = String::new();
        for c in name.to_lowercase().chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c);
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let slug = slug.trim_end_matches('-');
        let slug = if slug.is_empty() { "workflow" } else { slug };
        (1..)
            .map(|n| match n {
                1 => WorkflowId::new(format!("user.{slug}")),
                n => WorkflowId::new(format!("user.{slug}-{n}")),
            })
            .find(|id| !self.keys.contains_key(id.as_str()))
            .expect("some suffix is free")
    }

    /// The Editor if available, else the first available workflow, else the
    /// Editor anyway (it is never quarantined).
    pub fn default_workflow(&self) -> Option<&WorkflowDef> {
        let editor = WorkflowId::editor();
        self.available()
            .find(|def| def.id == editor)
            .or_else(|| self.available().next())
            .or_else(|| self.get(&editor))
    }
}

/// `layout.json` from before `state.json` (Phase 1): the active workflow and
/// each visited workflow's last layout (`S` is the GUI's dock state). Read
/// once to migrate; never written any more, so older builds keep reading it.
///
/// Builds from before workflows read `active` and `sections` only, keyed by
/// legacy ids, and fail on keys they do not know. So workflows with a legacy
/// id are written there under it, `active` always names one, and every other
/// workflow goes in `workflows` / `active_workflow`, which those builds ignore.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default, bound(deserialize = "S: Deserialize<'de>"))]
pub struct LayoutFile<S> {
    active: Option<String>,
    sections: BTreeMap<String, S>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_workflow: Option<WorkflowId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    workflows: BTreeMap<WorkflowId, S>,
}

impl<S> Default for LayoutFile<S> {
    fn default() -> Self {
        Self {
            active: None,
            sections: BTreeMap::new(),
            active_workflow: None,
            workflows: BTreeMap::new(),
        }
    }
}

impl<S> LayoutFile<S> {
    /// Layout state of `active` and the others, as one file. States of ids the
    /// registry does not know are kept (the workflow may come back).
    pub fn new(
        registry: &Registry,
        active: &WorkflowId,
        states: impl IntoIterator<Item = (WorkflowId, S)>,
    ) -> Self {
        let legacy = |id: &WorkflowId| {
            registry
                .get(id)
                .and_then(|def| def.legacy_ids.first().cloned())
        };
        let mut file = Self::default();
        for (id, state) in states {
            match legacy(&id) {
                Some(key) => {
                    file.sections.insert(key, state);
                }
                None => {
                    file.workflows.insert(id, state);
                }
            }
        }
        file.active = legacy(active).or_else(|| {
            registry
                .default_workflow()
                .and_then(|def| def.legacy_ids.first().cloned())
        });
        if file
            .active
            .as_ref()
            .and_then(|key| registry.resolve(key))
            .map(|def| &def.id)
            != Some(active)
        {
            file.active_workflow = Some(active.clone());
        }
        file
    }

    /// The active workflow (if it is known) and every saved state by id.
    pub fn into_parts(self, registry: &Registry) -> (Option<WorkflowId>, HashMap<WorkflowId, S>) {
        let resolve = |key: &str| {
            registry
                .resolve(key)
                .map(|def| def.id.clone())
                .unwrap_or_else(|| WorkflowId::new(key))
        };
        let mut states: HashMap<WorkflowId, S> = self.workflows.into_iter().collect();
        for (key, state) in self.sections {
            states.insert(resolve(&key), state);
        }
        let known = |id: &WorkflowId| registry.get(id).is_some();
        let active = self
            .active_workflow
            .filter(known)
            .or_else(|| self.active.as_deref().map(resolve).filter(known));
        (active, states)
    }
}
