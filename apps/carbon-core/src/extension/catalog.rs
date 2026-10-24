//! The extension catalog:
//! a git repository whose root holds `catalog.json`, an index of packages
//! and their versions. Every version is pinned: the git URL, the full commit
//! id, and the content hash of the package's files (`content_hash`), so what
//! installs is exactly what was listed.
//!
//! Signatures: the repository may hold `catalog.json.minisig`, made
//! with `minisign -Sm catalog.json`. Signing the index signs every pin, so
//! every package it lists. With trusted keys set, the signature is required
//! and must come from one of them; without keys the catalog is used and
//! shown as not checked.
//!
//! Icons: a package may name an image file in the catalog repository
//! (`icon`, `ICON_TYPES`) pinned by its SHA-256 (`icon_sha256`, so the
//! signature covers it). They are read from the same commit as
//! `catalog.json` and cached as `<sha256>.<type>`; a package without one
//! (or whose file does not match) shows a placeholder.
//!
//! The repository URL and the keys: `catalog.toml` in the preferences folder
//! ([`CatalogConfig`]); `CARBON_EXTENSION_CATALOG` overrides the URL.
//!
//! ```json
//! { "schema": 1, "packages": [ { "id": "acme.sketch", "name": "Sketch",
//!   "icon": "icons/sketch.png", "icon_sha256": "<64 hex>",
//!   "versions": [ { "version": "1.2.0", "git": "https://example.com/sketch.git",
//!     "commit": "<40 hex>", "hash": "sha256:<64 hex>" } ] } ] }
//! ```

use std::path::Path;

use minisign_verify::{PublicKey, Signature};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use sha2::{Digest, Sha256};

use super::{
    ExtensionId, FetchControl, MAX_ICON_BYTES, Package, Pin, check_icon_type, check_id,
    check_package_path, git, is_commit_id,
};

/// Carbon's official catalog (and extensions): the default catalog. Until it
/// is signed and its key is trusted, `carbon.*` extensions from it are
/// accepted on the strength of the URL (HTTPS to GitHub) and its pins.
pub const OFFICIAL_CATALOG: &str = "https://github.com/denizhoroz/carbon-extensions-basic";

/// Whether `url` is [`OFFICIAL_CATALOG`] (case, a trailing `/` or `.git` aside).
pub fn is_official_url(url: &str) -> bool {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    url.eq_ignore_ascii_case(OFFICIAL_CATALOG)
}

/// Overrides the catalog repository's URL.
pub const CATALOG_ENV: &str = "CARBON_EXTENSION_CATALOG";
/// Version of the `catalog.json` format this build reads.
pub const CATALOG_SCHEMA: u32 = 1;
/// Name of the index at the catalog repository's root.
pub const CATALOG_FILE: &str = "catalog.json";
/// Its minisign signature, next to it.
pub const CATALOG_SIGNATURE: &str = "catalog.json.minisig";
/// Largest `catalog.json` (or signature) read.
const MAX_CATALOG_BYTES: u64 = 4 * 1024 * 1024;

/// `catalog.toml`: the catalog repository and the keys trusted to sign it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// minisign public keys (the base64 line of a `.pub` file, or all of it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
}

impl CatalogConfig {
    /// The file's settings; defaults when there is no file.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|err| format!("{}: {err}", path.display())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(format!("{}: {err}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|err| err.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
        }
        std::fs::write(path, text).map_err(|err| format!("{}: {err}", path.display()))
    }

    /// The URL in use: `CARBON_EXTENSION_CATALOG`, else the file's, else
    /// [`OFFICIAL_CATALOG`].
    pub fn effective_url(&self) -> Option<String> {
        env_url()
            .or_else(|| {
                self.url
                    .as_deref()
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                    .map(str::to_string)
            })
            .or_else(|| Some(OFFICIAL_CATALOG.to_string()))
    }

    /// Problems with the keys (each must be a minisign public key).
    pub fn check(&self) -> Vec<String> {
        self.keys
            .iter()
            .enumerate()
            .filter_map(|(ix, key)| {
                parse_key(key)
                    .err()
                    .map(|err| format!("trusted key {}: {err}", ix + 1))
            })
            .collect()
    }
}

/// `CARBON_EXTENSION_CATALOG`, if set.
pub fn env_url() -> Option<String> {
    std::env::var(CATALOG_ENV)
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

/// The base64 line of a key given alone or as a whole `.pub` file.
fn key_line(key: &str) -> &str {
    key.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty() && !line.starts_with("untrusted comment:"))
        .unwrap_or_default()
}

fn parse_key(key: &str) -> Result<PublicKey, String> {
    PublicKey::from_base64(key_line(key)).map_err(|_| "not a minisign public key".to_string())
}

/// `catalog.json` and its signature as fetched (or as cached).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFiles {
    pub text: String,
    pub signature: Option<String>,
    /// The icons it names, fetched and checked (none when read from the
    /// cache: those are already in the icon folder).
    pub icons: Vec<CatalogIcon>,
}

/// A package icon fetched with the catalog; its bytes match `icon_sha256`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogIcon {
    /// [`CatalogPackage::icon_file_name`].
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// How a catalog's signature was judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signing {
    /// Signed by this trusted key (its first characters), with the
    /// signature's trusted comment.
    Trusted { key: String, comment: String },
    /// No trusted keys are set, so nothing was checked.
    NotChecked,
}

/// Checks `files` against `keys`. With keys, a signature from one of them is
/// required; without, nothing is checked.
pub fn check_signature(files: &CatalogFiles, keys: &[String]) -> Result<Signing, String> {
    if keys.is_empty() {
        return Ok(Signing::NotChecked);
    }
    let Some(signature) = &files.signature else {
        return Err(format!(
            "{CATALOG_FILE} is not signed, but trusted keys are set: \
             a signature ({CATALOG_SIGNATURE}) is required"
        ));
    };
    let signature = Signature::decode(signature)
        .map_err(|err| format!("{CATALOG_SIGNATURE} cannot be read: {err}"))?;
    for key in keys {
        let Ok(public) = parse_key(key) else {
            continue;
        };
        if public
            .verify(files.text.as_bytes(), &signature, false)
            .is_ok()
        {
            return Ok(Signing::Trusted {
                key: key_line(key).chars().take(12).collect(),
                comment: signature.trusted_comment().to_string(),
            });
        }
    }
    Err(format!(
        "{CATALOG_FILE} is not signed by a trusted key (its signature matches none of them)"
    ))
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub schema: u32,
    #[serde(default)]
    pub packages: Vec<CatalogPackage>,
    /// How its signature was judged (set by [`Catalog::load`]).
    #[serde(skip)]
    pub signing: Option<Signing>,
    /// Fetched from [`OFFICIAL_CATALOG`] (set by whoever knows the URL).
    #[serde(skip)]
    pub official: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogPackage {
    pub id: ExtensionId,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Image file in the catalog repository (`ICON_TYPES`, at most
    /// `MAX_ICON_BYTES`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// SHA-256 of the icon file, 64 lowercase hex digits; required with `icon`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_sha256: Option<String>,
    /// As in the manifest (`CATEGORIES`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    /// File endings and names its languages cover, so Carbon can suggest it
    /// when such a file opens without fetching the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suffixes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_names: Vec<String>,
    pub versions: Vec<CatalogVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogVersion {
    pub version: String,
    /// Repository URL.
    pub git: String,
    /// Full commit id.
    pub commit: String,
    /// The package's folder in the repository; the root when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
        if let Some(path) = &self.path {
            super::check_package_path(path)
                .map_err(|err| format!("{}: folder {err}", self.version))?;
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
    /// Name of its cached icon file, `<sha256>.<type>`; `None` without an icon.
    pub fn icon_file_name(&self) -> Option<String> {
        let kind = Path::new(self.icon.as_deref()?).extension()?.to_str()?;
        Some(format!(
            "{}.{}",
            self.icon_sha256.as_deref()?,
            kind.to_ascii_lowercase()
        ))
    }

    /// Problems with `icon` / `icon_sha256`.
    fn check_icon(&self) -> Result<(), String> {
        let Some(icon) = &self.icon else {
            return Ok(());
        };
        check_package_path(icon).map_err(|err| format!("icon {err}"))?;
        check_icon_type(icon)?;
        let sha_ok = self.icon_sha256.as_deref().is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
        if !sha_ok {
            return Err("icon needs `icon_sha256`: 64 lowercase hex digits".into());
        }
        Ok(())
    }

    /// Its icon read with `read`, when it is there, small enough and matches
    /// `icon_sha256`.
    fn read_icon(&self, read: &git::ReadFile<'_>) -> Option<CatalogIcon> {
        let file_name = self.icon_file_name()?;
        let bytes = read(self.icon.as_deref()?, MAX_ICON_BYTES).ok()??;
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        (Some(digest.as_str()) == self.icon_sha256.as_deref())
            .then_some(CatalogIcon { file_name, bytes })
    }

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
    /// reported; the rest stay usable. The signature is not looked at: see
    /// [`Catalog::load`].
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
            if let Err(problem) = package.check_icon() {
                problems.push(format!("`{id}` {problem}"));
                package.icon = None;
            }
            if package.icon.is_none() {
                package.icon_sha256 = None;
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

    /// Checks the signature against `keys`, then parses; a signature
    /// problem refuses the whole catalog.
    pub fn load(files: &CatalogFiles, keys: &[String]) -> Result<(Self, Vec<String>), String> {
        let signing = check_signature(files, keys)?;
        let (mut catalog, problems) = Self::parse(&files.text)?;
        catalog.signing = Some(signing);
        Ok((catalog, problems))
    }

    /// Signed by a trusted key.
    pub fn is_signed(&self) -> bool {
        matches!(self.signing, Some(Signing::Trusted { .. }))
    }

    /// May install `carbon.*` extensions: signed by a trusted key, or the
    /// official catalog (which, once keys are set, must be signed too).
    pub fn is_trusted(&self) -> bool {
        self.is_signed() || self.official
    }

    pub fn get(&self, id: &ExtensionId) -> Option<&CatalogPackage> {
        self.packages.iter().find(|package| &package.id == id)
    }
}

/// Fetches the catalog repository's `catalog.json`, its signature and the
/// icons it names (default branch). Blocking and network-bound: run it off
/// the UI thread.
pub fn fetch_catalog(url: &str, control: &FetchControl) -> Result<CatalogFiles, String> {
    let utf8 = |bytes: Vec<u8>, name: &str| {
        String::from_utf8(bytes).map_err(|_| format!("{name} is not UTF-8"))
    };
    git::read_head(url, control, |read| {
        let catalog = read(CATALOG_FILE, MAX_CATALOG_BYTES)?
            .ok_or_else(|| format!("{url}: no {CATALOG_FILE} at the root"))?;
        let text = utf8(catalog, CATALOG_FILE)?;
        let signature = read(CATALOG_SIGNATURE, MAX_CATALOG_BYTES)?
            .map(|bytes| utf8(bytes, CATALOG_SIGNATURE))
            .transpose()?;
        // Icons of a catalog that does not parse are not needed.
        let icons = Catalog::parse(&text)
            .map(|(catalog, _)| {
                catalog
                    .packages
                    .iter()
                    .filter_map(|package| package.read_icon(read))
                    .collect()
            })
            .unwrap_or_default();
        Ok(CatalogFiles {
            text,
            signature,
            icons,
        })
    })
}

/// Fetches a listed version at its pinned commit and reads it as a package
/// carrying the pin; `plan_install` checks id, version and content hash.
/// `signed`: the catalog was signed by a trusted key.
pub fn fetch_from_catalog(
    package: &CatalogPackage,
    version: &CatalogVersion,
    signed: bool,
    control: &FetchControl,
) -> Result<Package, Vec<String>> {
    let mut fetched = super::fetch_git_path(
        &version.git,
        &version.commit,
        version.path.as_deref(),
        control,
    )?;
    fetched.pin = Some(Pin {
        id: package.id.clone(),
        version: version.version.clone(),
        hash: version.hash.clone(),
        signed,
    });
    Ok(fetched)
}
