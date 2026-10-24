//! Reading and writing graph files. The only place that names the YAML crate,
//! so it can be swapped. Writing is canonical: the same document always gives
//! the same bytes, so saving an unchanged graph changes nothing in git.

use std::io::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{GRAPH_FORMAT, GraphDoc};

/// Parses a graph file.
pub fn from_yaml(text: &str) -> Result<GraphDoc, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc: GraphDoc = serde_yaml_ng::from_str(text).map_err(|err| err.to_string())?;
    if doc.format != GRAPH_FORMAT {
        return Err(format!(
            "carbon_graph {} is not supported (this Carbon reads {GRAPH_FORMAT})",
            doc.format
        ));
    }
    Ok(doc)
}

/// Writes a graph file.
pub fn to_yaml(doc: &GraphDoc) -> String {
    serde_yaml_ng::to_string(doc).expect("a graph is plain data")
}

/// Reads a graph file from disk.
pub fn read_file(path: &Path) -> Result<GraphDoc, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    from_yaml(&text).map_err(|err| format!("{}: {err}", path.display()))
}

/// Writes a graph file atomically (a temp file beside it, renamed over), so
/// a crash never leaves half a file. Creates the folder.
pub fn write_file(path: &Path, doc: &GraphDoc) -> Result<(), String> {
    let fail = |err: &dyn std::fmt::Display| format!("{}: {err}", path.display());
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|err| fail(&err))?;
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|err| fail(&err))?;
    temp.write_all(to_yaml(doc).as_bytes())
        .map_err(|err| fail(&err))?;
    temp.as_file().sync_all().map_err(|err| fail(&err))?;
    temp.persist(path).map_err(|err| fail(&err.error))?;
    Ok(())
}

/// SHA-256 of everything but the layout: equal when only nodes moved.
pub fn logic_hash(doc: &GraphDoc) -> String {
    let mut logic = doc.clone();
    logic.layout = Default::default();
    Sha256::digest(to_yaml(&logic).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
