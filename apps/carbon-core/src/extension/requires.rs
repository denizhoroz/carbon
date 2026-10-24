//! `[requires]`: other extensions an extension needs. Each maps an id to a
//! semver range:
//!
//! ```toml
//! [requires]
//! extensions = { "acme.base" = ">=1.0" }
//! ```
//!
//! An extension whose requirement is missing stays loaded but is not used,
//! and Manage Extensions says why, with an Install button when the catalog
//! has it. Nothing is installed without a click, and each dependency is shown
//! on its own (GlassWorm spread through VS Code's `extensionDependencies`).
//! `tools` and `mcp` are accepted so manifests can name them, and stay
//! missing until those systems exist.

use std::collections::BTreeMap;
use std::fmt;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use super::check_id;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    /// Extension id -> version range.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mcp: BTreeMap<String, String>,
}

/// What kind of thing a requirement names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequireKind {
    Extension,
    Tool,
    Mcp,
}

impl fmt::Display for RequireKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Extension => "extension",
            Self::Tool => "tool",
            Self::Mcp => "MCP server",
        })
    }
}

/// A requirement that is not met, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub kind: RequireKind,
    pub id: String,
    /// The version range asked for.
    pub range: String,
    /// Plain words: "not installed", "1.0.0 is installed", ...
    pub reason: String,
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Needs {} {} {}: {}",
            self.kind, self.id, self.range, self.reason
        )
    }
}

impl Requires {
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty() && self.tools.is_empty() && self.mcp.is_empty()
    }

    /// Every requirement as (kind, id, range).
    pub fn all(&self) -> impl Iterator<Item = (RequireKind, &str, &str)> {
        [
            (RequireKind::Extension, &self.extensions),
            (RequireKind::Tool, &self.tools),
            (RequireKind::Mcp, &self.mcp),
        ]
        .into_iter()
        .flat_map(|(kind, map)| {
            map.iter()
                .map(move |(id, range)| (kind, id.as_str(), range.as_str()))
        })
    }

    /// Problems that keep the manifest from loading: bad extension ids,
    /// ranges that do not parse, an extension requiring itself.
    pub fn check(&self, own: &str) -> Vec<String> {
        let mut problems = Vec::new();
        for (kind, id, range) in self.all() {
            if id.trim().is_empty() {
                problems.push(format!("requires: an empty {kind} id"));
            }
            if kind == RequireKind::Extension {
                if let Err(problem) = check_id(id) {
                    problems.push(format!("requires: {problem}"));
                } else if id == own {
                    problems.push(format!("requires: `{id}` cannot require itself"));
                }
            }
            if VersionReq::parse(range).is_err() {
                problems.push(format!(
                    "requires: {kind} `{id}`: `{range}` is not a version range"
                ));
            }
        }
        problems
    }

    /// The unmet requirements, given each extension's version (`found(id)`;
    /// `Err` says why it cannot be used).
    pub fn missing(&self, found: impl Fn(&str) -> Result<String, String>) -> Vec<Missing> {
        let mut missing = Vec::new();
        for (kind, id, range) in self.all() {
            let reason = match kind {
                RequireKind::Extension => match found(id) {
                    Err(reason) => Some(reason),
                    Ok(version) => {
                        let fits = VersionReq::parse(range).is_ok_and(|req| {
                            Version::parse(&version).is_ok_and(|v| req.matches(&v))
                        });
                        (!fits).then(|| format!("{version} is installed"))
                    }
                },
                _ => Some(format!("{kind}s are not supported by this Carbon yet")),
            };
            if let Some(reason) = reason {
                missing.push(Missing {
                    kind,
                    id: id.to_string(),
                    range: range.to_string(),
                    reason,
                });
            }
        }
        missing
    }
}
