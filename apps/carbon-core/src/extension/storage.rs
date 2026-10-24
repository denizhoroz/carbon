//! Extension storage: a key/value map per extension and open folder, kept in
//! `<preferences>/extensions/storage/<id>/<folder hash>.json`. Extension code
//! keeps state here between events (it can be reloaded at any time).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Largest value.
pub const MAX_VALUE: usize = 1024 * 1024;
/// Largest total of one map.
pub const MAX_TOTAL: usize = 16 * 1024 * 1024;

/// The map's file for `extension` in `workspace` (or with no folder open).
pub fn storage_file(root: &Path, extension: &str, workspace: Option<&Path>) -> PathBuf {
    let name = match workspace {
        Some(folder) => {
            let text = folder.display().to_string().to_lowercase();
            let digest = Sha256::digest(text.as_bytes());
            digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
        }
        None => "no-folder".to_string(),
    };
    root.join(extension).join(format!("{name}.json"))
}

fn load(file: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(file: &Path, map: &BTreeMap<String, String>) -> Result<(), String> {
    let fail = |err: &dyn std::fmt::Display| format!("{}: {err}", file.display());
    let dir = file.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|err| fail(&err))?;
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|err| fail(&err))?;
    let text = serde_json::to_string(map).map_err(|err| fail(&err))?;
    temp.write_all(text.as_bytes()).map_err(|err| fail(&err))?;
    temp.persist(file).map_err(|err| fail(&err.error))?;
    Ok(())
}

pub fn get(file: &Path, key: &str) -> Option<String> {
    load(file).remove(key)
}

pub fn set(file: &Path, key: &str, value: &str) -> Result<(), String> {
    if value.len() > MAX_VALUE {
        return Err(format!("a value is at most {} KiB", MAX_VALUE / 1024));
    }
    let mut map = load(file);
    map.insert(key.to_string(), value.to_string());
    let total: usize = map.iter().map(|(k, v)| k.len() + v.len()).sum();
    if total > MAX_TOTAL {
        return Err(format!("storage is full ({} MiB)", MAX_TOTAL / 1024 / 1024));
    }
    save(file, &map)
}

pub fn delete(file: &Path, key: &str) {
    let mut map = load(file);
    if map.remove(key).is_some() {
        _ = save(file, &map);
    }
}

pub fn keys(file: &Path, prefix: &str) -> Vec<String> {
    load(file)
        .into_keys()
        .filter(|key| key.starts_with(prefix))
        .collect()
}
