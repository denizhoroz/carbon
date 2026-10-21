//! The workflow catalog (Phase 4, workflow-system.md s4.5; user 2026-10-21):
//! a git repository whose root holds `catalog.json`, an index of packages
//! and their versions. Every version is pinned: the git URL, the full commit
//! id, and the content hash of the package's files (`content_hash`), so what
//! installs is exactly what was listed. Its URL comes from
//! `CARBON_WORKFLOW_CATALOG` until Settings can set it (like Q16).
//!
//! ```json
//! { "schema": 1, "packages": [ { "id": "acme.sketch", "name": "Sketch",
//!   "versions": [ { "version": "1.2.0", "git": "https://example.com/sketch.git",
//!     "commit": "<40 hex>", "hash": "sha256:<64 hex>" } ] } ] }
//! ```

use std::sync::atomic::AtomicBool;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use super::{Package, Pin, WorkflowId, check_id, fetch_git, git, is_commit_id};

/// The catalog repository's URL.
pub const CATALOG_ENV: &str = "CARBON_WORKFLOW_CATALOG";
/// Version of the `catalog.json` format this build reads.
pub const CATALOG_SCHEMA: u32 = 1;
/// Name of the index at the catalog repository's root.
const CATALOG_FILE: &str = "catalog.json";
/// Largest `catalog.json` read.
const MAX_CATALOG_BYTES: u64 = 4 * 1024 * 1024;

/// `CARBON_WORKFLOW_CATALOG`, if set.
pub fn catalog_url() -> Option<String> {
    std::env::var(CATALOG_ENV)
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub schema: u32,
    #[serde(default)]
    pub packages: Vec<CatalogPackage>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogPackage {
    pub id: WorkflowId,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Lucide icon name, as in a manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub versions: Vec<CatalogVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogVersion {
    pub version: String,
    /// Repository URL.
    pub git: String,
    /// Full commit id.
    pub commit: String,
    /// `content_hash` of the package at that commit.
    pub hash: String,
}

impl CatalogVersion {
    fn check(&self) -> Result<Version, String> {
        let version = Version::parse(&self.version)
            .map_err(|_| format!("version `{}` is not semver", self.version))?;
        if self.git.trim().is_empty() {
            return Err(format!("{}: no git URL", self.version));
        }
        if !is_commit_id(&self.commit) {
            return Err(format!("{}: commit must be a full commit id", self.version));
        }
        let hash_ok = self.hash.strip_prefix("sha256:").is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
        if !hash_ok {
            return Err(format!(
                "{}: hash must be `sha256:` and 64 lowercase hex digits",
                self.version
            ));
        }
        Ok(version)
    }
}

impl CatalogPackage {
    /// The highest version whose number fits `range` (any when `None`).
    pub fn best(&self, range: Option<&VersionReq>) -> Option<&CatalogVersion> {
        self.versions
            .iter()
            .filter_map(|entry| Some((Version::parse(&entry.version).ok()?, entry)))
            .filter(|(version, _)| range.is_none_or(|range| range.matches(version)))
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, entry)| entry)
    }
}

impl Catalog {
    /// Parses `catalog.json`. Entries with problems are left out and
    /// reported; the rest stay usable.
    pub fn parse(text: &str) -> Result<(Self, Vec<String>), String> {
        let mut catalog: Self =
            serde_json::from_str(text).map_err(|err| format!("{CATALOG_FILE}: {err}"))?;
        if catalog.schema != CATALOG_SCHEMA {
            return Err(format!(
                "{CATALOG_FILE}: schema {} is not supported (this Carbon reads {CATALOG_SCHEMA})",
                catalog.schema
            ));
        }
        let mut problems = Vec::new();
        let mut seen = std::collections::HashSet::new();
        catalog.packages.retain_mut(|package| {
            let id = package.id.clone();
            if let Err(problem) = check_id(id.as_str()) {
                problems.push(problem);
                return false;
            }
            if !seen.insert(id.clone()) {
                problems.push(format!("`{id}` is listed twice"));
                return false;
            }
            package.versions.retain(|entry| match entry.check() {
                Ok(_) => true,
                Err(problem) => {
                    problems.push(format!("`{id}` {problem}"));
                    false
                }
            });
            !package.versions.is_empty()
        });
        Ok((catalog, problems))
    }

    pub fn get(&self, id: &WorkflowId) -> Option<&CatalogPackage> {
        self.packages.iter().find(|package| &package.id == id)
    }
}

/// Fetches the catalog repository's `catalog.json` (default branch).
/// Blocking and network-bound: run it off the UI thread.
pub fn fetch_catalog(url: &str, interrupt: &AtomicBool) -> Result<String, String> {
    let bytes = git::fetch_file(url, CATALOG_FILE, MAX_CATALOG_BYTES, interrupt)?;
    String::from_utf8(bytes).map_err(|_| format!("{CATALOG_FILE} is not UTF-8"))
}

/// Fetches a listed version at its pinned commit and reads it as a package
/// carrying the pin; `plan_install` checks id, version and content hash.
pub fn fetch_from_catalog(
    package: &CatalogPackage,
    version: &CatalogVersion,
    interrupt: &AtomicBool,
) -> Result<Package, Vec<String>> {
    let mut fetched = fetch_git(&version.git, &version.commit, interrupt)?;
    fetched.pin = Some(Pin {
        id: package.id.clone(),
        version: version.version.clone(),
        hash: version.hash.clone(),
    });
    Ok(fetched)
}
