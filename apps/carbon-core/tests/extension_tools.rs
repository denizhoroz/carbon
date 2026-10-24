//! Downloaded tools: checking a download against the manifest's SHA-256,
//! unpacking it, and the server table preferring PATH over the download.

use std::fs;
use std::io::Write;
use std::path::Path;

use carbon_core::extension::{
    ExtensionDirs, ExtensionId, Registry, StateFile, ToolProgress, current_target, download_tool,
    downloaded_program, install_tool_file,
};
use sha2::{Digest, Sha256};

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// An extension with a server `fake-ls` whose download for this computer is
/// `archive` with `hash`.
fn extension(
    root: &Path,
    archive: &str,
    hash: &str,
    bin: Option<&str>,
) -> (ExtensionDirs, Registry) {
    let target = current_target().expect("a known computer");
    let bin = bin
        .map(|bin| format!("bin = \"{bin}\"\n"))
        .unwrap_or_default();
    let manifest = format!(
        r#"
id = "acme.fake"
name = "Fake"
version = "1.0.0"
schema_version = 1
carbon_api = ">=0.1, <0.2"
capabilities = ["process.exec:fake-ls", "net:example.com"]

[[languages]]
id = "fake"
name = "Fake"
suffixes = ["fake"]

[[language_servers]]
id = "fake-ls"
name = "Fake LS"
languages = ["fake"]
command = "fake-ls"

[language_servers.download.{target}]
url = "https://example.com/fake-ls.{archive}"
sha256 = "{hash}"
version = "1.2.3"
archive = "{archive}"
{bin}"#
    );
    let package = root.join("pkg");
    write(&package.join("extension.toml"), manifest.as_bytes());
    let dirs = ExtensionDirs::new(root.join("prefs"));
    let mut state = StateFile::default();
    state.dev.insert(package);
    let registry = Registry::load(Some(&dirs), &state);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    (dirs, registry)
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_checked_zip_unpacks_and_the_table_uses_it() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = zip_bytes(&[("bin/fake-ls.exe", b"program"), ("README", b"x")]);
    let (dirs, registry) = extension(temp.path(), "zip", &sha256(&bytes), Some("bin/fake-ls.exe"));
    let loaded = registry.get(&ExtensionId::new("acme.fake")).unwrap();
    let server = &loaded.def.language_servers[0];
    assert!(downloaded_program(&dirs, loaded, server).is_none());

    let file = temp.path().join("download.zip");
    write(&file, &bytes);
    let program = install_tool_file(&dirs, loaded, server, &file).unwrap();
    assert_eq!(fs::read(&program).unwrap(), b"program");
    assert!(program.ends_with("acme.fake/fake-ls/1.2.3/bin/fake-ls.exe"));
    assert_eq!(
        downloaded_program(&dirs, loaded, server),
        Some(program.clone())
    );

    // Not on PATH, so the table's server resolves to the downloaded copy.
    let table = registry.server_table(Some(&dirs), |_, _| None);
    let config = table.server("fake").unwrap();
    assert_eq!(config.downloaded.as_deref(), Some(program.as_path()));
    assert_eq!(config.download.as_ref().unwrap().host, "example.com");
    assert_eq!(config.resolve(), Some(program));
}

#[test]
fn a_wrong_hash_installs_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let (dirs, registry) = extension(temp.path(), "gz", &"0".repeat(64), None);
    let loaded = registry.get(&ExtensionId::new("acme.fake")).unwrap();
    let server = &loaded.def.language_servers[0];
    let file = temp.path().join("download.gz");
    write(&file, b"not the pinned file");
    let err = install_tool_file(&dirs, loaded, server, &file).unwrap_err();
    assert!(err.contains("SHA-256"), "{err}");
    assert!(downloaded_program(&dirs, loaded, server).is_none());
    // No staging folders left behind.
    let base = dirs.tools_dir().join("acme.fake/fake-ls");
    assert_eq!(fs::read_dir(base).map(|e| e.count()).unwrap_or(0), 0);
}

#[test]
fn a_gzip_becomes_the_program_and_bad_zips_are_refused() {
    let temp = tempfile::tempdir().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(b"gzipped program").unwrap();
    let bytes = gz.finish().unwrap();
    let (dirs, registry) = extension(temp.path(), "gz", &sha256(&bytes), None);
    let loaded = registry.get(&ExtensionId::new("acme.fake")).unwrap();
    let server = &loaded.def.language_servers[0];
    let file = temp.path().join("download.gz");
    write(&file, &bytes);
    let program = install_tool_file(&dirs, loaded, server, &file).unwrap();
    assert_eq!(fs::read(&program).unwrap(), b"gzipped program");
    let name = if cfg!(windows) {
        "fake-ls.exe"
    } else {
        "fake-ls"
    };
    assert!(program.ends_with(name));

    let temp = tempfile::tempdir().unwrap();
    let slip = zip_bytes(&[("../evil.exe", b"x")]);
    let (dirs, registry) = extension(temp.path(), "zip", &sha256(&slip), Some("evil.exe"));
    let loaded = registry.get(&ExtensionId::new("acme.fake")).unwrap();
    let file = temp.path().join("slip.zip");
    write(&file, &slip);
    assert!(install_tool_file(&dirs, loaded, &loaded.def.language_servers[0], &file).is_err());
    assert!(!temp.path().join("prefs/tools/acme.fake/evil.exe").exists());
}

/// The real rust-analyzer build the official Rust extension pins, downloaded
/// over HTTPS and run.
#[test]
#[ignore = "needs the network; downloads ~15 MB"]
fn the_rust_extension_downloads_rust_analyzer() {
    let temp = tempfile::tempdir().unwrap();
    let dirs = ExtensionDirs::new(temp.path());
    let mut state = StateFile::default();
    // A checkout of the official extensions repo, `carbon-extensions-basic/` in
    // the repo root (ignored by git).
    let rust =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../carbon-extensions-basic/languages/rust");
    let rust = dunce::canonicalize(&rust)
        .unwrap_or_else(|_| panic!("{} missing: clone carbon-extensions-basic", rust.display()));
    state.dev.insert(rust);
    let registry = Registry::load(Some(&dirs), &state);
    let loaded = registry.get(&ExtensionId::new("carbon.rust")).unwrap();
    let server = &loaded.def.language_servers[0];
    let progress = ToolProgress::default();
    let program = download_tool(&dirs, loaded, server, &progress).unwrap();
    let (received, total) = progress.bytes();
    assert!(received > 1_000_000 && (total == 0 || total == received));
    let output = std::process::Command::new(&program)
        .arg("--version")
        .output()
        .unwrap();
    let version = String::from_utf8_lossy(&output.stdout);
    assert!(version.starts_with("rust-analyzer"), "{version}");
}
