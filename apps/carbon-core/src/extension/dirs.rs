//! Where extension files live:
//!
//! - Preferences, in the user config dir (`settings::user_config_dir`):
//!   `extensions/state.json` (flags, grants, install records, dev links),
//!   `extensions/catalog.toml`, the cached `catalog.json`.
//! - Installed packages, in the OS local data dir:
//!   `Carbon/extensions/<id>/<version>/`, read-only, replaced on update.
//! - Downloaded tools (language servers): `Carbon/tools/<server>/<version>/`.
//!
//! `CARBON_EXTENSIONS_DIR` puts all of it in one folder (tests, click tests);
//! with only `CARBON_CONFIG_DIR` set, the data folders go under it too, so a
//! test with a scratch config touches nothing real.

use std::path::{Path, PathBuf};

use semver::Version;

use super::{MANIFEST, Origin};

/// Puts preferences, installed packages and tools in this folder.
pub const DIR_ENV: &str = "CARBON_EXTENSIONS_DIR";

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// The OS local data dir: `%LOCALAPPDATA%` on Windows,
/// `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` (or
/// `~/.local/share`) elsewhere.
fn local_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env_dir("LOCALAPPDATA")
    }
    #[cfg(target_os = "macos")]
    {
        env_dir("HOME").map(|home| home.join("Library/Application Support"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        env_dir("XDG_DATA_HOME").or_else(|| env_dir("HOME").map(|home| home.join(".local/share")))
    }
}

#[derive(Clone, Debug)]
pub struct ExtensionDirs {
    /// Preferences: state, catalog settings and cache.
    root: PathBuf,
    /// Installed packages.
    installed: PathBuf,
    /// Downloaded tools.
    tools: PathBuf,
}

impl ExtensionDirs {
    /// Everything under `root`: preferences in it, packages in
    /// `root/installed`, tools in `root/tools`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            installed: root.join("installed"),
            tools: root.join("tools"),
            root,
        }
    }

    /// `CARBON_EXTENSIONS_DIR`, else preferences in
    /// `<config>/extensions` and data in the local data dir (under the config
    /// dir when `CARBON_CONFIG_DIR` is set). `None` without a config dir.
    pub fn from_env() -> Option<Self> {
        if let Some(root) = env_dir(DIR_ENV) {
            return Some(Self::new(root));
        }
        let config = crate::settings::user_config_dir()?;
        let mut dirs = Self::new(config.join("extensions"));
        if env_dir("CARBON_CONFIG_DIR").is_none()
            && let Some(local) = local_data_dir()
        {
            let data = local.join("Carbon");
            dirs.installed = data.join("extensions");
            dirs.tools = data.join("tools");
        }
        Some(dirs)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn installed_dir(&self) -> &Path {
        &self.installed
    }

    pub fn tools_dir(&self) -> &Path {
        &self.tools
    }

    pub fn state_file(&self) -> PathBuf {
        self.root.join("state.json")
    }

    /// The last catalog fetched, shown until the next refresh; its signature
    /// sits next to it (`catalog.json.minisig`).
    pub fn catalog_file(&self) -> PathBuf {
        self.root.join("catalog.json")
    }

    /// The cached catalog's package icons, `<sha256>.<type>`.
    pub fn catalog_icons_dir(&self) -> PathBuf {
        self.root.join("catalog-icons")
    }

    /// The catalog repository and trusted keys (`CatalogConfig`).
    pub fn catalog_config_file(&self) -> PathBuf {
        self.root.join("catalog.toml")
    }

    /// The newest version of each installed package:
    /// `installed/<id>/<version>/extension.toml`, the highest semver folder
    /// that has a manifest. Folders not named by a version are skipped
    /// (staging folders of an install in progress).
    pub fn installed_manifests(&self) -> Vec<(PathBuf, Result<String, String>)> {
        sorted_entries(&self.installed)
            .into_iter()
            .filter(|path| path.is_dir())
            .filter_map(|package| {
                sorted_entries(&package)
                    .into_iter()
                    .filter_map(|dir| {
                        let version = Version::parse(dir.file_name()?.to_str()?).ok()?;
                        let manifest = dir.join(MANIFEST);
                        manifest.is_file().then_some((version, manifest))
                    })
                    .max_by(|a, b| a.0.cmp(&b.0))
                    .map(|(_, manifest)| manifest)
            })
            .map(|path| {
                let text =
                    std::fs::read_to_string(&path).map_err(|err| format!("cannot read: {err}"));
                (path, text)
            })
            .collect()
    }

    /// Deletes an installed package (all its versions). Refuses anything
    /// outside the installed folder; a development folder is only unlinked
    /// (the caller drops it from `StateFile::dev`), never deleted.
    pub fn remove(&self, origin: &Origin, package_dir: &Path) -> Result<(), String> {
        if *origin == Origin::Dev {
            return Err("a development folder is unlinked, not deleted".into());
        }
        let target = package_dir.parent().ok_or("no folder to remove")?;
        let (Ok(target), Ok(inside)) = (
            dunce::canonicalize(target),
            dunce::canonicalize(&self.installed),
        ) else {
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
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    paths.sort();
    paths
}
