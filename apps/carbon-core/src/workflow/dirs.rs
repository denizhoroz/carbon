//! Where workflow files live (`workflow-system.md` s4.7, Q15-Q16):
//!
//! - Shipped packages, Carbon's own (read-only): `<shipped>/<name>/workflow.toml`
//!   (`shipped_dir`).
//! - Installed packages, downloaded by the user: `<installed>/<id>/<version>/`,
//!   in the OS local data dir.
//! - Preferences and the workflow list, in the OS config dir:
//!   `<root>/user/<dir>/workflow.toml`, `<root>/overlay/<id>.toml`,
//!   `<root>/state/` (`state.rs`).
//!
//! Each location has an environment override until Settings can change them
//! (tests and click tests use them too).

use std::path::{Path, PathBuf};

use semver::Version;

use super::{Origin, WorkflowDef};

/// Overrides the preferences root (and, unless `INSTALLED_ENV` is set, puts
/// installed packages under it too, so a test touches nothing real).
pub const DIR_ENV: &str = "CARBON_WORKFLOWS_DIR";
/// Overrides the installed-packages folder.
pub const INSTALLED_ENV: &str = "CARBON_INSTALLED_WORKFLOWS_DIR";
/// Overrides the shipped-packages folder.
pub const SHIPPED_ENV: &str = "CARBON_SHIPPED_WORKFLOWS_DIR";
/// Name of the shipped folder next to the binary.
const SHIPPED_DIR_NAME: &str = "carbon-workflow";
/// The repo's `apps/carbon-workflow` (development runs).
const REPO_SHIPPED_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../carbon-workflow");

/// The shipped-packages folder: `CARBON_SHIPPED_WORKFLOWS_DIR`, else
/// `carbon-workflow/` next to the binary, else the repo's
/// `apps/carbon-workflow` (development); `None` when none exists.
pub fn shipped_dir() -> Option<PathBuf> {
    if let Some(dir) = env_dir(SHIPPED_ENV) {
        return Some(dir);
    }
    let beside_binary = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(SHIPPED_DIR_NAME)));
    beside_binary
        .into_iter()
        .chain([PathBuf::from(REPO_SHIPPED_DIR)])
        .find(|dir| dir.is_dir())
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

#[derive(Clone, Debug)]
pub struct WorkflowDirs {
    /// Preferences and the workflow list.
    root: PathBuf,
    /// Installed packages.
    installed: PathBuf,
}

impl WorkflowDirs {
    /// Preferences in `root`, installed packages in `root/installed`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            installed: root.join("installed"),
            root,
        }
    }

    pub fn with_installed(mut self, installed: impl Into<PathBuf>) -> Self {
        self.installed = installed.into();
        self
    }

    /// Preferences: `CARBON_WORKFLOWS_DIR`, else `<config dir>/Carbon/workflows`.
    /// Installed: `CARBON_INSTALLED_WORKFLOWS_DIR`, else under the preferences
    /// override when one is set, else `<local data dir>/Carbon/workflows/installed`.
    /// `None` when the OS has no config dir.
    pub fn from_env() -> Option<Self> {
        let overridden = env_dir(DIR_ENV);
        let dirs = match &overridden {
            Some(root) => Self::new(root),
            None => {
                let mut dirs = Self::new(::dirs::config_dir()?.join("Carbon").join("workflows"));
                if let Some(local) = ::dirs::data_local_dir() {
                    dirs.installed = local.join("Carbon").join("workflows").join("installed");
                }
                dirs
            }
        };
        Some(match env_dir(INSTALLED_ENV) {
            Some(installed) => dirs.with_installed(installed),
            None => dirs,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn installed_dir(&self) -> &Path {
        &self.installed
    }

    pub fn user_dir(&self) -> PathBuf {
        self.root.join("user")
    }

    pub fn overlay_dir(&self) -> PathBuf {
        self.root.join("overlay")
    }

    /// Overlay file of a workflow.
    pub fn overlay_file(&self, id: &str) -> PathBuf {
        self.overlay_dir().join(format!("{id}.toml"))
    }

    pub fn state_file(&self) -> PathBuf {
        self.root.join("state").join("state.json")
    }

    /// The last catalog fetched (`catalog.rs`), shown until the next refresh;
    /// its signature sits next to it (`catalog.json.minisig`).
    pub fn catalog_file(&self) -> PathBuf {
        self.root.join("state").join("catalog.json")
    }

    /// The catalog repository and trusted keys (`CatalogConfig`).
    pub fn catalog_config_file(&self) -> PathBuf {
        self.root.join("catalog.toml")
    }

    /// Exists while a workflow is being opened (crash-loop guard).
    pub fn entering_file(&self) -> PathBuf {
        self.root.join("state").join("entering")
    }

    /// Every `user/<dir>/workflow.toml`, sorted by folder name.
    pub fn user_manifests(&self) -> Vec<(PathBuf, Result<String, String>)> {
        package_manifests(&self.user_dir())
    }

    /// The newest version of each installed package:
    /// `installed/<id>/<version>/workflow.toml`, the highest semver folder
    /// that has a manifest. Folders not named by a version are skipped.
    pub fn installed_manifests(&self) -> Vec<(PathBuf, Result<String, String>)> {
        sorted_entries(&self.installed)
            .into_iter()
            .filter(|path| path.is_dir())
            .filter_map(|package| {
                sorted_entries(&package)
                    .into_iter()
                    .filter_map(|dir| {
                        let version = Version::parse(dir.file_name()?.to_str()?).ok()?;
                        let manifest = dir.join("workflow.toml");
                        manifest.is_file().then_some((version, manifest))
                    })
                    .max_by(|a, b| a.0.cmp(&b.0))
                    .map(|(_, manifest)| manifest)
            })
            .map(|path| {
                let text = read(&path);
                (path, text)
            })
            .collect()
    }

    /// Every `overlay/<id>.toml` as (path, id, text).
    pub fn overlays(&self) -> Vec<(PathBuf, String, Result<String, String>)> {
        sorted_entries(&self.overlay_dir())
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .filter_map(|path| {
                let id = path.file_stem()?.to_str()?.to_string();
                let text = read(&path);
                Some((path, id, text))
            })
            .collect()
    }

    /// Writes a new user workflow to `user/<folder>/workflow.toml`, the
    /// folder named after the id's last part (`-2`, `-3`, ... when taken).
    /// `package`: the original's package folder; its other files are copied
    /// too (pages, assets), so the copy works on its own.
    pub fn create_user_workflow(
        &self,
        def: &WorkflowDef,
        package: Option<&Path>,
    ) -> Result<PathBuf, String> {
        let text = def.to_toml()?;
        let base = def.id.as_str().rsplit('.').next().unwrap_or("workflow");
        let user = self.user_dir();
        let folder = (1..)
            .map(|n| match n {
                1 => user.join(base),
                n => user.join(format!("{base}-{n}")),
            })
            .find(|dir| !dir.exists())
            .expect("some suffix is free");
        let path = folder.join("workflow.toml");
        std::fs::create_dir_all(&folder)
            .and_then(|_| std::fs::write(&path, text))
            .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
        if let Some(package) = package {
            super::copy_package_files(package, &folder)?;
        }
        Ok(path)
    }

    /// Deletes a user workflow's folder or an installed package (all its
    /// versions). Refuses anything outside the user and installed folders.
    pub fn remove_workflow(&self, origin: &Origin) -> Result<(), String> {
        let (target, inside) = match origin {
            Origin::User(manifest) => (manifest.parent(), self.user_dir()),
            Origin::Installed(manifest) => (
                manifest.parent().and_then(Path::parent),
                self.installed.clone(),
            ),
            Origin::Builtin => return Err("Carbon's own workflows cannot be removed".into()),
        };
        let target = target.ok_or("no folder to remove")?;
        let (Ok(target), Ok(inside)) = (dunce::canonicalize(target), dunce::canonicalize(&inside))
        else {
            return Err(format!("{} does not exist", target.display()));
        };
        if target == inside || !target.starts_with(&inside) {
            return Err(format!(
                "{} is not inside {}",
                target.display(),
                inside.display()
            ));
        }
        std::fs::remove_dir_all(&target)
            .map_err(|err| format!("cannot remove {}: {err}", target.display()))
    }

    /// Deletes a workflow's overlay ("reset to default"). Missing is fine.
    pub fn remove_overlay(&self, id: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.overlay_file(id)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }
}

/// Every `<dir>/<folder>/workflow.toml`, sorted by folder name, with its text
/// or why it could not be read. A missing `dir` means none.
pub(super) fn package_manifests(dir: &Path) -> Vec<(PathBuf, Result<String, String>)> {
    sorted_entries(dir)
        .into_iter()
        .filter(|path| path.is_dir())
        .map(|dir| dir.join("workflow.toml"))
        .filter(|path| path.is_file())
        .map(|path| {
            let text = read(&path);
            (path, text)
        })
        .collect()
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    paths.sort();
    paths
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| format!("cannot read: {err}"))
}
