//! `state/state.json`: the active workflow, each workflow's last layout,
//! disabled and quarantined workflows. Separate from the manifests, so
//! disabling never deletes files. The layout itself is the GUI's dock state,
//! kept as JSON here so one broken entry only resets that workflow.
//!
//! Crash-loop guard: `state/entering` names the workflow being opened and is
//! deleted once it opened. Found at start, it means the last opening never
//! finished.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{WorkflowDef, WorkflowDirs, WorkflowId};

/// Version of the state file format.
pub const STATE_SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StateFile {
    pub schema: u32,
    pub active: Option<WorkflowId>,
    pub disabled: BTreeSet<WorkflowId>,
    /// Workflow -> why it was quarantined.
    pub quarantined: BTreeMap<WorkflowId, String>,
    pub workflows: BTreeMap<WorkflowId, SavedWorkflow>,
    /// Each workflow's named layouts and auto-hide flags (the GUI's
    /// `LayoutStore` as JSON).
    pub layouts: BTreeMap<WorkflowId, serde_json::Value>,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            schema: STATE_SCHEMA,
            active: None,
            disabled: BTreeSet::new(),
            quarantined: BTreeMap::new(),
            workflows: BTreeMap::new(),
            layouts: BTreeMap::new(),
        }
    }
}

/// One workflow's last layout and the default it was made from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWorkflow {
    /// The GUI's dock state.
    pub dock: serde_json::Value,
    /// `WorkflowDef::layout_fingerprint` of the default at save time.
    pub base_fingerprint: String,
    /// `WorkflowDef::base_panels` at save time (for a later merge).
    #[serde(default)]
    pub base_panels: Vec<String>,
}

impl SavedWorkflow {
    pub fn new(def: &WorkflowDef, dock: serde_json::Value) -> Self {
        Self {
            dock,
            base_fingerprint: def.layout_fingerprint(),
            base_panels: def.base_panels(),
        }
    }

    /// Made from the workflow's current default layout.
    pub fn matches(&self, def: &WorkflowDef) -> bool {
        self.base_fingerprint == def.layout_fingerprint()
    }
}

impl StateFile {
    /// `Ok(None)` when there is no file; `Err` when it is unreadable or not
    /// a state file (the caller starts fresh and keeps the bad file).
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(format!("cannot read {}: {err}", path.display())),
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|err| format!("{} is not valid: {err}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|err| err.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        }
        // Write then rename, so a crash mid-write never leaves half a file.
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, json)
            .and_then(|_| std::fs::rename(&temp, path))
            .map_err(|err| format!("cannot write {}: {err}", path.display()))
    }

    /// Moves an unreadable state file aside (`state.json.bad`) so it is not
    /// overwritten and can be inspected.
    pub fn set_aside(path: &Path) {
        let _ = std::fs::rename(path, path.with_extension("json.bad"));
    }
}

/// Marks `id` as being opened.
pub fn start_entering(dirs: &WorkflowDirs, id: &WorkflowId) -> std::io::Result<()> {
    let path = dirs.entering_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, id.as_str())
}

/// The opening finished.
pub fn finish_entering(dirs: &WorkflowDirs) {
    let _ = std::fs::remove_file(dirs.entering_file());
}

/// A workflow whose opening never finished last time, if any; clears the mark.
pub fn take_unfinished(dirs: &WorkflowDirs) -> Option<WorkflowId> {
    let path = dirs.entering_file();
    let id = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let id = id.trim();
    (!id.is_empty()).then(|| WorkflowId::new(id))
}
