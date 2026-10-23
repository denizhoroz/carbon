//! Installing extension packages (from the shelved workflow branch).
//!
//! A package is a folder, a zip or a git commit (`git.rs`) whose root holds
//! `extension.toml` (a zip may wrap it in one top folder). [`read_package`]
//! checks everything before anything is written: the manifest, every path
//! (package rules, no links, no leaving the package: also stops "zip slip"),
//! file count and size limits, files the manifest names that exist; and
//! computes the content hash. [`plan_install`] compares it with what is
//! loaded: new, update, reinstall or downgrade; capabilities to ask for;
//! clashes; a catalog pin that does not match; requirements not met.
//! [`ExtensionDirs::install`] copies into `installed/<id>/<version>/` through
//! a staging folder, checks the hash again, and removes the other versions,
//! so the installed version is always the one the user picked. Nothing in a
//! package is run while installing.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ExtensionDef, ExtensionDirs, ExtensionId, GitCheckout, MANIFEST, Missing, OFFICIAL_PREFIX,
    Origin, Registry, check_package_path, resolve_package_file,
};

/// Most files a package may hold.
pub const MAX_PACKAGE_FILES: usize = 10_000;
/// Largest total size of a package's files, unpacked.
pub const MAX_PACKAGE_BYTES: u64 = 256 * 1024 * 1024;

/// Where a package comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallSource {
    Folder(PathBuf),
    Zip(PathBuf),
    /// A fetched git commit, written out to a temporary folder.
    Git(GitCheckout),
}

impl InstallSource {
    /// A `.zip` file or else a folder.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let zip = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
        if zip && path.is_file() {
            Self::Zip(path)
        } else {
            Self::Folder(path)
        }
    }

    /// The folder the files are read from (a zip: none).
    fn folder(&self) -> Option<&Path> {
        match self {
            Self::Folder(path) => Some(path),
            Self::Git(checkout) => Some(&checkout.root),
            Self::Zip(_) => None,
        }
    }

    /// For people: the path, or `url @ rev (commit)`.
    pub fn describe(&self) -> String {
        match self {
            Self::Folder(path) | Self::Zip(path) => path.display().to_string(),
            Self::Git(checkout) => checkout.to_string(),
        }
    }
}

/// What a package must be: a catalog entry (`catalog.rs`), or the record of
/// a pinned install when reinstalling it. The commit is checked while
/// fetching; these are checked by [`plan_install`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub id: ExtensionId,
    pub version: String,
    /// `content_hash` format.
    pub hash: String,
    /// The pin comes from a catalog signed by a trusted key.
    pub signed: bool,
}

/// A package that passed every check, ready to plan and install.
#[derive(Clone, Debug)]
pub struct Package {
    pub def: ExtensionDef,
    pub source: InstallSource,
    /// Package paths of its files, sorted (`extension.toml` included).
    pub files: Vec<String>,
    /// Total size, unpacked.
    pub bytes: u64,
    /// `sha256:<hex>` over its files ([`content_hash`]).
    pub hash: String,
    /// Set when installing from the catalog or reinstalling a pinned package.
    pub pin: Option<Pin>,
    /// Zip: name prefix of the package root (`""` or `"folder/"`).
    prefix: String,
}

impl Package {
    /// What `state.json` keeps about this install.
    pub fn record(&self) -> InstallRecord {
        let (source, rev, commit, path, commit_pinned) = match &self.source {
            InstallSource::Git(checkout) => (
                checkout.url.clone(),
                Some(checkout.rev.clone()).filter(|rev| *rev != checkout.commit),
                Some(checkout.commit.clone()),
                checkout.path.clone(),
                checkout.rev == checkout.commit,
            ),
            source => (source.describe(), None, None, None, false),
        };
        InstallRecord {
            source,
            rev,
            commit,
            path,
            hash: self.hash.clone(),
            pinned: self.pin.is_some() || commit_pinned,
            signed: self.pin.as_ref().is_some_and(|pin| pin.signed),
        }
    }
}

/// Where an installed package came from (`StateFile::installs`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    /// Folder or zip path, or git URL.
    pub source: String,
    /// Git: the tag or branch asked for (none when a commit id was given).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    /// Git: the commit installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Git: the package's folder in the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// [`content_hash`] of the installed files.
    pub hash: String,
    /// Pinned by the catalog (commit and content hash) or by a git commit id:
    /// a trust level above a folder, zip, tag or branch (s4.5).
    #[serde(default)]
    pub pinned: bool,
    /// The pin came from a catalog signed by a trusted key (above pinned).
    #[serde(default)]
    pub signed: bool,
}

/// Checks an installed package's folder against the hash recorded at
/// install.
pub fn verify_installed(dir: &Path, hash: &str) -> Result<(), String> {
    let now = installed_hash(dir)?;
    if now == hash {
        Ok(())
    } else {
        Err("its files changed since it was installed".into())
    }
}

/// The content hash of an installed package's folder as it is now.
pub fn installed_hash(dir: &Path) -> Result<String, String> {
    let (files, _) = list_folder(dir).map_err(|problems| problems.join("; "))?;
    content_hash(dir, &files)
}

/// Reads and checks a package; every problem found, not just the first.
pub fn read_package(source: InstallSource) -> Result<Package, Vec<String>> {
    let (files, bytes, prefix, manifest, hash) = match &source {
        InstallSource::Zip(path) => read_zip(path)?,
        source => {
            let root = source.folder().expect("a folder or a git checkout");
            let (files, bytes) = list_folder(root)?;
            let manifest = fs::read_to_string(root.join(MANIFEST)).map_err(|_| {
                vec![format!(
                    "no {MANIFEST} at the root of {}",
                    source.describe()
                )]
            })?;
            let hash = content_hash(root, &files).map_err(|err| vec![err])?;
            (files, bytes, String::new(), manifest, hash)
        }
    };
    let def = match ExtensionDef::parse(&manifest) {
        Ok((def, _warnings)) => def,
        Err(err) => return Err(vec![format!("{MANIFEST}: {err}")]),
    };
    let mut problems = def.validate();
    for path in def.files() {
        if check_package_path(path).is_ok() && files.binary_search(&path.to_string()).is_err() {
            problems.push(format!(
                "`{path}` is not in the package (names are case-sensitive)"
            ));
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Package {
        def,
        source,
        files,
        bytes,
        hash,
        pin: None,
        prefix,
    })
}

/// `sha256:<hex>` over a package's files: for each package path in sorted
/// order, the path, a NUL byte and the file's own SHA-256. The same files
/// give the same hash whatever the source (folder, zip, git), so a catalog
/// can pin it. `files` as from [`list_folder`].
pub fn content_hash(root: &Path, files: &[String]) -> Result<String, String> {
    let mut digests = BTreeMap::new();
    for path in files {
        let file = resolve_package_file(root, path)?;
        let fail = |err: std::io::Error| format!("{}: {err}", file.display());
        let mut hasher = Sha256::new();
        std::io::copy(&mut fs::File::open(&file).map_err(fail)?, &mut hasher).map_err(fail)?;
        digests.insert(path.clone(), hasher.finalize().into());
    }
    Ok(combine(&digests))
}

fn combine(digests: &BTreeMap<String, [u8; 32]>) -> String {
    let mut hasher = Sha256::new();
    for (path, digest) in digests {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(digest);
    }
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{hex}")
}

/// Every file under `root` as package paths, and their total size. Links are
/// refused; a `.git` folder at the root is left out.
pub fn list_folder(root: &Path) -> Result<(Vec<String>, u64), Vec<String>> {
    let mut files = Vec::new();
    let mut bytes = 0;
    let mut problems = Vec::new();
    walk(root, "", &mut files, &mut bytes, &mut problems);
    if files.len() > MAX_PACKAGE_FILES {
        problems.push(format!("more than {MAX_PACKAGE_FILES} files"));
    }
    if bytes > MAX_PACKAGE_BYTES {
        problems.push(format!(
            "larger than {} MB",
            MAX_PACKAGE_BYTES / 1024 / 1024
        ));
    }
    if problems.is_empty() {
        files.sort();
        Ok((files, bytes))
    } else {
        Err(problems)
    }
}

fn walk(
    dir: &Path,
    prefix: &str,
    files: &mut Vec<String>,
    bytes: &mut u64,
    problems: &mut Vec<String>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => return problems.push(format!("{}: {err}", dir.display())),
    };
    for entry in entries.filter_map(Result::ok) {
        // Stop early on huge trees; the limit check reports it.
        if files.len() > MAX_PACKAGE_FILES {
            return;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if prefix.is_empty() && name == ".git" {
            continue;
        }
        let path = format!("{prefix}{name}");
        let Ok(kind) = entry.file_type() else {
            problems.push(format!("`{path}`: cannot read"));
            continue;
        };
        if kind.is_symlink() {
            problems.push(format!("`{path}`: links are not allowed in a package"));
        } else if kind.is_dir() {
            walk(&entry.path(), &format!("{path}/"), files, bytes, problems);
        } else if let Err(problem) = check_package_path(&path) {
            problems.push(problem);
        } else {
            *bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            files.push(path);
        }
    }
}

/// Files, bytes, prefix, manifest text, content hash.
type ZipContents = (Vec<String>, u64, String, String, String);

fn read_zip(path: &Path) -> Result<ZipContents, Vec<String>> {
    let fail = |err: &dyn std::fmt::Display| vec![format!("{}: {err}", path.display())];
    let file = fs::File::open(path).map_err(|err| fail(&err))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|err| fail(&err))?;
    let names = archive
        .file_names()
        .map(|name| name.map(|name| entry_name(&name)))
        .collect::<Result<Vec<String>, _>>()
        .map_err(|err| fail(&err))?;
    let prefix = zip_prefix(&names).ok_or_else(|| {
        vec![format!(
            "no {MANIFEST} at the root of {} (or in its one top folder)",
            path.display()
        )]
    })?;
    let mut files = Vec::new();
    let mut bytes = 0u64;
    let mut digests = BTreeMap::new();
    let mut problems = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|err| fail(&err))?;
        let full = entry_name(&entry.name().map_err(|err| fail(&err))?);
        let Some(name) = full.strip_prefix(prefix.as_str()) else {
            problems.push(format!("`{full}` is outside the package folder"));
            continue;
        };
        if entry.is_dir() || name.is_empty() {
            continue;
        }
        if entry.is_symlink() {
            problems.push(format!("`{name}`: links are not allowed in a package"));
        } else if let Err(problem) = check_package_path(name) {
            problems.push(problem);
        } else if bytes <= MAX_PACKAGE_BYTES {
            // Hash what is really there, up to the limit, whatever the header says.
            let name = name.to_string();
            let mut hasher = Sha256::new();
            bytes += std::io::copy(
                &mut (&mut entry).take(MAX_PACKAGE_BYTES - bytes + 1),
                &mut hasher,
            )
            .map_err(|err| fail(&err))?;
            digests.insert(name.clone(), hasher.finalize().into());
            files.push(name);
        }
    }
    if files.len() > MAX_PACKAGE_FILES {
        problems.push(format!("more than {MAX_PACKAGE_FILES} files"));
    }
    if bytes > MAX_PACKAGE_BYTES {
        problems.push(format!(
            "larger than {} MB",
            MAX_PACKAGE_BYTES / 1024 / 1024
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut manifest = String::new();
    let manifest_index = names
        .iter()
        .position(|name| *name == format!("{prefix}{MANIFEST}"))
        .ok_or_else(|| vec![format!("no {MANIFEST} in {}", path.display())])?;
    archive
        .by_index(manifest_index)
        .map_err(|err| fail(&err))?
        .take(1024 * 1024)
        .read_to_string(&mut manifest)
        .map_err(|err| fail(&err))?;
    files.sort();
    Ok((files, bytes, prefix, manifest, combine(&digests)))
}

/// A zip entry's name with `\` read as `/`: Windows PowerShell's
/// `Compress-Archive` writes `assets\x.png`. The package path checks run
/// after this, so `..\x` is still refused.
fn entry_name(name: &str) -> String {
    name.replace('\\', "/")
}

/// `""` when the manifest is at the zip's root, `"top/"` when every entry is
/// inside one folder that holds it.
fn zip_prefix(names: &[String]) -> Option<String> {
    if names.iter().any(|name| name == MANIFEST) {
        return Some(String::new());
    }
    let top = names.first()?.split('/').next()?.to_string();
    let prefix = format!("{top}/");
    let inside = names.iter().all(|name| name.starts_with(&prefix));
    (inside
        && names
            .iter()
            .any(|name| *name == format!("{prefix}{MANIFEST}")))
    .then_some(prefix)
}

/// What installing a package would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallKind {
    New,
    Update { from: String },
    Reinstall,
    Downgrade { from: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallPlan {
    pub kind: InstallKind,
    /// Asked for and not granted yet: the user is asked about these.
    pub new_capabilities: Vec<String>,
    /// Asked for and already granted.
    pub kept_capabilities: Vec<String>,
    /// Granted before, no longer asked for (dropped on install).
    pub dropped_capabilities: Vec<String>,
    /// Requirements the loaded extensions do not meet: it installs, but
    /// stays off until they are.
    pub needs: Vec<Missing>,
    /// Reasons it cannot be installed (an id clash, a catalog pin that does
    /// not match).
    pub problems: Vec<String>,
}

/// Compares `package` with the loaded extensions and the capabilities
/// granted to its id so far.
pub fn plan_install(
    package: &Package,
    registry: &Registry,
    granted: &BTreeSet<String>,
) -> InstallPlan {
    let def = &package.def;
    let mut problems = Vec::new();
    let mut kind = InstallKind::New;
    match registry.get(&def.id) {
        Some(old) if old.origin == Origin::Installed => {
            let new = Version::parse(&def.version).ok();
            let old_version = Version::parse(&old.def.version).ok();
            kind = match new.cmp(&old_version) {
                std::cmp::Ordering::Greater => InstallKind::Update {
                    from: old.def.version.clone(),
                },
                std::cmp::Ordering::Equal => InstallKind::Reinstall,
                std::cmp::Ordering::Less => InstallKind::Downgrade {
                    from: old.def.version.clone(),
                },
            };
        }
        Some(_) => problems.push(format!(
            "`{}` is linked for development: unlink it first",
            def.id
        )),
        None => {}
    }
    if def.id.is_official() && !package.pin.as_ref().is_some_and(|pin| pin.signed) {
        problems.push(format!(
            "`{OFFICIAL_PREFIX}*` extensions install only from a catalog signed by a trusted key"
        ));
    }
    if let Some(pin) = &package.pin {
        if pin.id != def.id {
            problems.push(format!(
                "expected `{}` (pinned) but the package is `{}`",
                pin.id, def.id
            ));
        }
        if pin.version != def.version {
            problems.push(format!(
                "expected version {} (pinned) but the package is {}",
                pin.version, def.version
            ));
        }
        if pin.hash != package.hash {
            problems.push(
                "its files differ from the pinned ones (the content hash does not match)".into(),
            );
        }
    }
    let asked: BTreeSet<String> = def.capabilities.iter().cloned().collect();
    InstallPlan {
        kind,
        new_capabilities: asked.difference(granted).cloned().collect(),
        kept_capabilities: asked.intersection(granted).cloned().collect(),
        dropped_capabilities: granted.difference(&asked).cloned().collect(),
        needs: registry.unmet(def),
        problems,
    }
}

impl ExtensionDirs {
    /// Copies `package` into `installed/<id>/<version>/` and removes the
    /// package's other versions; returns the installed manifest. Files are
    /// checked again while copying, and the copy must have the hash that was
    /// read (the source may have changed).
    pub fn install(&self, package: &Package) -> Result<PathBuf, String> {
        let def = &package.def;
        let id_dir = self.installed_dir().join(def.id.as_str());
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|time| time.as_nanos())
            .unwrap_or_default();
        let staging = id_dir.join(format!(".staging-{}-{stamp}", std::process::id()));
        let result = copy_package(package, &staging).and_then(|_| {
            if content_hash(&staging, &package.files)? != package.hash {
                return Err(format!(
                    "{} changed since it was read",
                    package.source.describe()
                ));
            }
            let target = id_dir.join(&def.version);
            let old = id_dir.join(format!(".old-{stamp}"));
            if target.exists() {
                fs::rename(&target, &old).map_err(|err| format!("{}: {err}", target.display()))?;
            }
            fs::rename(&staging, &target).map_err(|err| format!("{}: {err}", staging.display()))?;
            Ok(target)
        });
        let target = match result {
            Ok(target) => target,
            Err(err) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(err);
            }
        };
        // Other versions and leftovers from earlier attempts.
        if let Ok(entries) = fs::read_dir(&id_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path != target && path.is_dir() {
                    let _ = fs::remove_dir_all(&path);
                }
            }
        }
        Ok(target.join(MANIFEST))
    }
}

fn copy_package(package: &Package, to: &Path) -> Result<(), String> {
    let write = |path: &str, bytes: &[u8]| -> Result<(), String> {
        let target = to.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        fs::write(&target, bytes).map_err(|err| format!("{}: {err}", target.display()))
    };
    fs::create_dir_all(to).map_err(|err| format!("{}: {err}", to.display()))?;
    match &package.source {
        InstallSource::Folder(root) | InstallSource::Git(GitCheckout { root, .. }) => {
            for path in &package.files {
                let file = resolve_package_file(root, path)?;
                let bytes = fs::read(&file).map_err(|err| format!("{}: {err}", file.display()))?;
                write(path, &bytes)?;
            }
        }
        InstallSource::Zip(zip_path) => {
            let fail = |err: &dyn std::fmt::Display| format!("{}: {err}", zip_path.display());
            let file = fs::File::open(zip_path).map_err(|err| fail(&err))?;
            let mut archive = zip::ZipArchive::new(file).map_err(|err| fail(&err))?;
            let mut total = 0u64;
            let mut copied = 0;
            for index in 0..archive.len() {
                let entry = archive.by_index(index).map_err(|err| fail(&err))?;
                let full = entry_name(&entry.name().map_err(|err| fail(&err))?);
                let Some(path) = full.strip_prefix(package.prefix.as_str()) else {
                    continue;
                };
                if entry.is_dir() || package.files.binary_search(&path.to_string()).is_err() {
                    continue;
                }
                check_package_path(path)?;
                copied += 1;
                if entry.is_symlink() {
                    return Err(format!("`{path}`: links are not allowed in a package"));
                }
                // Read at most what is left of the limit, whatever the header says.
                let mut bytes = Vec::new();
                entry
                    .take(MAX_PACKAGE_BYTES - total + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|err| fail(&err))?;
                total += bytes.len() as u64;
                if total > MAX_PACKAGE_BYTES {
                    return Err(format!(
                        "larger than {} MB unpacked",
                        MAX_PACKAGE_BYTES / 1024 / 1024
                    ));
                }
                write(path, &bytes)?;
            }
            if copied != package.files.len() {
                return Err(format!("{} changed since it was read", zip_path.display()));
            }
        }
    }
    Ok(())
}
