//! `extensions/state.json`: what the user turned off, what Carbon turned off,
//! granted capabilities, install records and linked development folders.
//! Separate from the packages, so turning one off never deletes files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ExtensionId, InstallRecord};

/// Version of the state file format.
pub const STATE_SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StateFile {
    pub schema: u32,
    pub disabled: BTreeSet<ExtensionId>,
    /// Extension -> why Carbon turned it off (it failed to load or run).
    pub quarantined: BTreeMap<ExtensionId, String>,
    /// Capabilities the user granted, per extension.
    pub grants: BTreeMap<ExtensionId, BTreeSet<String>>,
    /// Where each installed package came from, its content hash and pin.
    pub installs: BTreeMap<ExtensionId, InstallRecord>,
    /// Installed packages turned off because their files no longer match
    /// `installs` (extension -> why), until Reinstall or Keep Changes.
    pub changed: BTreeMap<ExtensionId, String>,
    /// Development folders loaded in place.
    pub dev: BTreeSet<PathBuf>,
    /// The first-run note about extensions was shown.
    pub welcomed: bool,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            schema: STATE_SCHEMA,
            disabled: BTreeSet::new(),
            quarantined: BTreeMap::new(),
            grants: BTreeMap::new(),
            installs: BTreeMap::new(),
            changed: BTreeMap::new(),
            dev: BTreeSet::new(),
            welcomed: false,
        }
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

    /// The file at `path`, or a fresh state when it is missing; an unreadable
    /// file is moved aside (`state.json.bad`) and the reason returned.
    pub fn load_or_default(path: &Path) -> (Self, Option<String>) {
        match Self::load(path) {
            Ok(state) => (state.unwrap_or_default(), None),
            Err(err) => {
                Self::set_aside(path);
                (Self::default(), Some(err))
            }
        }
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

    /// Whether the user granted `id` network access to `host` (a `net:`
    /// grant naming it, a `*.domain` above it, or `*`).
    pub fn allows_net(&self, id: &ExtensionId, host: &str) -> bool {
        self.grants.get(id).is_some_and(|grants| {
            grants.iter().any(|grant| {
                grant.strip_prefix("net:").is_some_and(|scope| {
                    scope == "*"
                        || scope == host
                        || scope
                            .strip_prefix("*.")
                            .is_some_and(|domain| host.ends_with(&format!(".{domain}")))
                })
            })
        })
    }

    /// Forgets an extension that was removed (its grants, record, flags).
    pub fn forget(&mut self, id: &ExtensionId) {
        self.disabled.remove(id);
        self.quarantined.remove(id);
        self.grants.remove(id);
        self.installs.remove(id);
        self.changed.remove(id);
    }
}
