//! Workflows: switchable app modes, one Section Bar icon each, described by a
//! `workflow.toml` manifest (`project/not_built/workflow-system.md`).
//!
//! A manifest names its default layout in panel *kinds* (`editor`, `chat`,
//! ...); the GUI turns kinds into panels. Built-in workflows are embedded in
//! the binary. User and installed workflows, overlays and the Manage Workflows
//! dialog come later (phases 2-4); this module is plain data with no UI types.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

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

/// Built-in manifests as (file name, text), in no particular order.
const BUILTINS: [(&str, &str); 3] = [
    ("editor.toml", include_str!("workflow/editor.toml")),
    (
        "paradigm-editor.toml",
        include_str!("workflow/paradigm-editor.toml"),
    ),
    (
        "agent-automation.toml",
        include_str!("workflow/agent-automation.toml"),
    ),
];

/// Manifest keys from the design that this build accepts but does not use yet.
const RESERVED_KEYS: [&str; 11] = [
    "derived_from",
    "commands",
    "menus",
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
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One `workflow.toml`.
#[derive(Clone, Debug, Deserialize)]
pub struct WorkflowDef {
    pub id: WorkflowId,
    pub schema_version: u32,
    /// Semver of the workflow itself.
    pub version: String,
    /// Semver range of the host contract, e.g. `">=0.1, <0.2"`.
    pub carbon_api: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Lucide icon name (`code`, `bot`, ...); the GUI falls back to a generic one.
    #[serde(default)]
    pub icon: Option<String>,
    /// Section Bar position, ascending.
    #[serde(default = "default_order")]
    pub order: i32,
    /// Keys this workflow had in saved files before it became a workflow.
    #[serde(default)]
    pub legacy_ids: Vec<String>,
    /// Shows an "open a folder" state without one (not used yet).
    #[serde(default)]
    pub requires_workspace: bool,
    #[serde(default)]
    pub layout: Layout,
    /// Panels by reference name; layout regions list these names.
    #[serde(default)]
    pub panels: BTreeMap<String, PanelRule>,
    /// Every other top-level key: reserved ones are ignored, the rest warned about.
    #[serde(flatten)]
    other: toml::Table,
}

fn default_order() -> i32 {
    1000
}

/// Default layout: the center and the three docks. A missing dock starts empty and closed.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub center: Option<Region>,
    pub left: Option<Region>,
    pub right: Option<Region>,
    pub bottom: Option<Region>,
}

/// One region of the layout: its panels as tabs, in order.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// Names from the manifest's `[panels]` table.
    #[serde(default)]
    pub panels: Vec<String>,
    /// Width (left/right) or height (bottom) in pixels; the host default if unset.
    pub size: Option<f32>,
    #[serde(default = "yes")]
    pub open: bool,
}

/// A panel the workflow uses.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelRule {
    /// Panel kind; defaults to the reference name.
    pub kind: Option<String>,
    /// Tab title; required for placeholders.
    pub title: Option<String>,
    /// More than one may be open (not enforced yet).
    #[serde(default)]
    pub multi: bool,
    /// The user may close it (not enforced yet).
    #[serde(default = "yes")]
    pub closable: bool,
}

fn yes() -> bool {
    true
}

impl WorkflowDef {
    /// Parses a manifest. Warnings name top-level keys this build does not know.
    pub fn parse(text: &str) -> Result<(Self, Vec<String>), String> {
        let def: Self = toml::from_str(text).map_err(|err| err.to_string())?;
        let warnings = def
            .other
            .keys()
            .filter(|key| !RESERVED_KEYS.contains(&key.as_str()))
            .map(|key| format!("unknown key `{key}` ignored"))
            .collect();
        Ok((def, warnings))
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

    /// Problems that keep this workflow from loading; empty when valid.
    /// `panel_kinds`: kinds the host can build. `builtin`: may use `carbon.*`.
    pub fn validate(&self, panel_kinds: &[&str], builtin: bool) -> Vec<String> {
        let mut problems = Vec::new();
        if self.schema_version != SCHEMA_VERSION {
            problems.push(format!(
                "schema_version {} is not supported (this Carbon reads {SCHEMA_VERSION})",
                self.schema_version
            ));
        }
        if let Err(problem) = check_id(self.id.as_str()) {
            problems.push(problem);
        } else if self.id.as_str().starts_with(BUILTIN_PREFIX) && !builtin {
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
            let kind = self.panel_kind(name);
            if !panel_kinds.contains(&kind) {
                problems.push(format!("panel `{name}`: kind `{kind}` is not available"));
            }
            if kind == PLACEHOLDER_KIND && rule.title.as_deref().is_none_or(str::is_empty) {
                problems.push(format!("panel `{name}`: a placeholder needs a title"));
            }
        }
        problems
    }
}

/// `publisher.name`: two or more dot-separated parts of `a-z`, `0-9`, `-`,
/// each starting with a letter or digit.
fn check_id(id: &str) -> Result<(), String> {
    let parts: Vec<&str> = id.split('.').collect();
    let part_ok = |part: &&str| {
        part.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    if parts.len() >= 2 && parts.iter().all(part_ok) {
        Ok(())
    } else {
        Err(format!(
            "id `{id}` must look like `publisher.name` (lowercase letters, digits, `-`)"
        ))
    }
}

/// A manifest that did not load, with why.
#[derive(Clone, Debug)]
pub struct Rejected {
    /// File name of the manifest.
    pub source: String,
    pub problems: Vec<String>,
}

/// The loaded workflows, sorted by `order` then id.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    workflows: Vec<WorkflowDef>,
    rejected: Vec<Rejected>,
    warnings: Vec<(String, String)>,
}

impl Registry {
    /// The built-in workflows.
    pub fn builtin(panel_kinds: &[&str]) -> Self {
        Self::from_sources(&BUILTINS, panel_kinds, true)
    }

    /// Loads manifests given as (file name, text). A bad one is rejected with
    /// its reasons and never stops the others; a duplicate id or legacy id
    /// rejects the later file.
    pub fn from_sources(sources: &[(&str, &str)], panel_kinds: &[&str], builtin: bool) -> Self {
        let mut registry = Self::default();
        let mut keys: HashMap<String, String> = HashMap::new();
        for (source, text) in sources {
            let reject = |problems| Rejected {
                source: source.to_string(),
                problems,
            };
            let (def, warnings) = match WorkflowDef::parse(text) {
                Ok(parsed) => parsed,
                Err(err) => {
                    registry.rejected.push(reject(vec![err]));
                    continue;
                }
            };
            let mut problems = def.validate(panel_kinds, builtin);
            for key in
                std::iter::once(def.id.as_str()).chain(def.legacy_ids.iter().map(String::as_str))
            {
                if let Some(owner) = keys.get(key) {
                    problems.push(format!("id `{key}` is already used by {owner}"));
                }
            }
            if !problems.is_empty() {
                registry.rejected.push(reject(problems));
                continue;
            }
            for key in std::iter::once(def.id.to_string()).chain(def.legacy_ids.iter().cloned()) {
                keys.insert(key, source.to_string());
            }
            registry.warnings.extend(
                warnings
                    .into_iter()
                    .map(|warning| (source.to_string(), warning)),
            );
            registry.workflows.push(def);
        }
        registry
            .workflows
            .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        registry
    }

    pub fn workflows(&self) -> &[WorkflowDef] {
        &self.workflows
    }

    pub fn rejected(&self) -> &[Rejected] {
        &self.rejected
    }

    /// (file name, warning) for keys that were ignored.
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

    /// The Editor if loaded, else the first workflow.
    pub fn default_workflow(&self) -> Option<&WorkflowDef> {
        self.get(&WorkflowId::editor())
            .or_else(|| self.workflows.first())
    }
}

/// `layout.json`: the active workflow and each visited workflow's last layout
/// (`S` is the GUI's dock state).
///
/// Builds from before workflows read `active` and `sections` only, keyed by
/// legacy ids, and fail on keys they do not know. So workflows with a legacy
/// id are written there under it, `active` always names one, and every other
/// workflow goes in `workflows` / `active_workflow`, which those builds ignore.
/// Going back to an older build keeps the Editor's layout.
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
