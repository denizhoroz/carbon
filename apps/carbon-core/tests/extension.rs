//! Extension manifests and the registry: validation rules, contributions
//! (languages by file, grammars, servers, settings, commands, panels), flags
//! and `[requires]`.

use std::fs;
use std::path::{Path, PathBuf};

use carbon_core::extension::{
    Blocked, ExtensionDef, ExtensionDirs, ExtensionId, PanelKind, Registry, RequireKind, StateFile,
    current_target,
};

const HEAD: &str = r#"
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"
"#;

fn manifest(id: &str, body: &str) -> String {
    format!("id = \"{id}\"\nname = \"{id}\"\n{HEAD}\n{body}")
}

fn problems(id: &str, body: &str) -> Vec<String> {
    let (def, _) = ExtensionDef::parse(&manifest(id, body)).expect("parses");
    def.validate()
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

const RUST: &str = r#"
categories = ["language"]
capabilities = ["process.exec:rust-analyzer", "net:github.com"]

[[languages]]
id = "rust"
name = "Rust"
suffixes = ["rs"]
grammar = "rust"
line_comment = "//"

[[grammars]]
name = "rust"
wasm = "grammars/rust.wasm"
highlights = "grammars/rust/highlights.scm"

[[language_servers]]
id = "rust-analyzer"
name = "rust-analyzer"
languages = ["rust"]
command = "rust-analyzer"
install_hint = "rustup component add rust-analyzer"

[language_servers.download.windows-x64]
url = "https://github.com/rust-lang/rust-analyzer/releases/download/2026-10-20/rust-analyzer-x86_64-pc-windows-msvc.zip"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
version = "2026-10-20"
archive = "zip"
bin = "rust-analyzer.exe"

[[settings]]
key = "check.command"
label = "Check command"
description = "What rust-analyzer runs on save."
kind = "choice"
default = "check"
choices = [ { value = "check", label = "cargo check" }, { value = "clippy", label = "cargo clippy" } ]
"#;

/// A package folder holding `body`'s manifest and empty files for every
/// path it names.
fn package(root: &Path, id: &str, body: &str) -> PathBuf {
    let dir = root.join(id);
    let text = manifest(id, body);
    write(&dir.join("extension.toml"), &text);
    let (def, _) = ExtensionDef::parse(&text).unwrap();
    for file in def.files() {
        write(&dir.join(file), "");
    }
    dir
}

#[test]
fn a_language_extension_is_valid() {
    assert!(
        problems("carbon.rust", RUST).is_empty(),
        "{:?}",
        problems("carbon.rust", RUST)
    );
    let (def, warnings) = ExtensionDef::parse(&manifest("carbon.rust", RUST)).unwrap();
    assert!(warnings.is_empty());
    let server = &def.language_servers[0];
    assert_eq!(
        server.download["windows-x64"].bin.as_deref(),
        Some("rust-analyzer.exe")
    );
    let setting = def.settings[0].to_setting(&def).unwrap();
    assert_eq!(setting.key, "carbon.rust.check.command");
    assert_eq!(setting.category, ["carbon.rust"]);
    assert_eq!(setting.extension.as_deref(), Some("carbon.rust"));
}

#[test]
fn servers_need_their_capabilities() {
    let no_exec = RUST.replace("\"process.exec:rust-analyzer\", ", "");
    assert!(
        problems("acme.rust", &no_exec)
            .iter()
            .any(|p| p.contains("process.exec:rust-analyzer"))
    );
    let no_net = RUST.replace(", \"net:github.com\"", "");
    assert!(
        problems("acme.rust", &no_net)
            .iter()
            .any(|p| p.contains("net:github.com"))
    );
    let wildcard = no_net.replace(
        "process.exec:rust-analyzer\"",
        "process.exec:rust-analyzer\", \"net:*.github.com\"",
    );
    // `*.github.com` does not cover `github.com` itself.
    assert!(
        problems("acme.rust", &wildcard)
            .iter()
            .any(|p| p.contains("net:github.com"))
    );
    let http = RUST.replace("url = \"https://", "url = \"http://");
    assert!(
        problems("acme.rust", &http)
            .iter()
            .any(|p| p.contains("https://"))
    );
    let path = RUST.replace("command = \"rust-analyzer\"", "command = \"C:/bin/ra.exe\"");
    assert!(
        problems("acme.rust", &path)
            .iter()
            .any(|p| p.contains("program name"))
    );
    let target = RUST.replace("download.windows-x64", "download.amiga-68k");
    assert!(
        problems("acme.rust", &target)
            .iter()
            .any(|p| p.contains("amiga-68k"))
    );
    let hash = RUST.replace(&"0".repeat(64), "abc");
    assert!(
        problems("acme.rust", &hash)
            .iter()
            .any(|p| p.contains("sha256"))
    );
    let language = RUST.replace("languages = [\"rust\"]", "languages = [\"rsut\"]");
    assert!(
        problems("acme.rust", &language)
            .iter()
            .any(|p| p.contains("rsut"))
    );
}

#[test]
fn bad_manifests_say_why() {
    assert!(
        problems("Bad Id", "")
            .iter()
            .any(|p| p.contains("publisher.name"))
    );
    let wrong_api = manifest("acme.x", "").replace(">=0.1, <0.2", ">=9");
    let (def, _) = ExtensionDef::parse(&wrong_api).unwrap();
    assert!(def.validate()[0].contains("needs Carbon API"));
    assert!(problems("acme.x", "categories = [\"games\"]")[0].contains("category"));
    let setting = r#"
[[settings]]
key = "size"
label = "Size"
kind = "integer"
default = "big"
"#;
    assert!(problems("acme.x", setting)[0].contains("default"));
    let command = r#"
[[commands]]
id = "go"
title = "Go"
"#;
    assert!(problems("acme.x", command)[0].contains("wasm"));
    let panel = r#"
[[panels]]
id = "home"
title = "Home"
kind = "view"
view = "missing"
"#;
    assert!(problems("acme.x", panel)[0].contains("no view"));
    let escape = r#"
[[themes]]
file = "../theme.json"
"#;
    assert!(!problems("acme.x", escape).is_empty());
    let (_, warnings) = ExtensionDef::parse(&manifest("acme.x", "colour = 1")).unwrap();
    assert!(warnings[0].contains("colour"));
}

#[test]
fn files_are_found_by_language_with_the_longest_suffix() {
    let temp = tempfile::tempdir().unwrap();
    let rust = package(temp.path(), "carbon.rust", RUST);
    let ts = package(
        temp.path(),
        "acme.ts",
        r#"
[[languages]]
id = "typescript"
name = "TypeScript"
suffixes = ["ts"]
[[languages]]
id = "typescript-def"
name = "TypeScript definitions"
suffixes = ["d.ts"]
[[languages]]
id = "make"
name = "Make"
file_names = ["Makefile"]
"#,
    );
    let rust_text = fs::read_to_string(rust.join("extension.toml")).unwrap();
    let ts_text = fs::read_to_string(ts.join("extension.toml")).unwrap();
    let registry = Registry::from_sources(&[(&rust, &rust_text), (&ts, &ts_text)]);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    let lang = |name: &str| {
        registry
            .language_for_path(Path::new(name))
            .map(|(_, lang)| lang.id.clone())
    };
    assert_eq!(lang("src/main.rs").as_deref(), Some("rust"));
    assert_eq!(lang("MAIN.RS").as_deref(), Some("rust"));
    assert_eq!(lang("a.ts").as_deref(), Some("typescript"));
    assert_eq!(lang("lib.d.ts").as_deref(), Some("typescript-def"));
    assert_eq!(lang("Makefile").as_deref(), Some("make"));
    assert_eq!(lang("rs"), None, "a suffix needs a dot before it");
    assert_eq!(lang("notes.txt"), None);

    assert_eq!(registry.grammars().len(), 1);
    let (loaded, server) = &registry.language_servers()[0];
    assert_eq!(
        (loaded.def.id.as_str(), server.command.as_str()),
        ("carbon.rust", "rust-analyzer")
    );
    assert_eq!(registry.settings()[0].key, "carbon.rust.check.command");
}

#[test]
fn missing_files_reject_the_extension() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("acme.rust");
    let text = manifest("acme.rust", RUST);
    write(&dir.join("extension.toml"), &text);
    let registry = Registry::from_sources(&[(&dir, &text)]);
    assert!(registry.all().is_empty());
    assert!(registry.rejected()[0].problems[0].contains("not in the package"));
}

#[test]
fn commands_and_panels_have_global_keys() {
    let temp = tempfile::tempdir().unwrap();
    let body = r#"
categories = ["panel"]
[[commands]]
id = "open-home"
title = "Open Home"
panel = "home"
[[menus]]
menu = "View"
command = "open-home"
[[panels]]
id = "home"
title = "Home"
kind = "view"
view = "home"
explorer_section = true
[[views.home]]
type = "heading"
text = "Hello"
[[views.home]]
type = "button"
label = "Again"
command = "open-home"
"#;
    let dir = package(temp.path(), "acme.home", body);
    let text = fs::read_to_string(dir.join("extension.toml")).unwrap();
    let registry = Registry::from_sources(&[(&dir, &text)]);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    let (_, panel) = registry.panel("acme.home/home").unwrap();
    assert_eq!((panel.kind, panel.placement()), (PanelKind::View, "left"));
    let (_, command) = registry.command("acme.home/open-home").unwrap();
    assert_eq!(command.panel.as_deref(), Some("home"));
    assert!(registry.panel("acme.home/nope").is_none());
}

#[test]
fn flags_and_requirements_block_extensions() {
    let temp = tempfile::tempdir().unwrap();
    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
    let base = package(temp.path(), "acme.base", "");
    let app = package(
        temp.path(),
        "acme.app",
        "[requires]\nextensions = { \"acme.base\" = \">=2.0\" }",
    );
    let addon = package(
        temp.path(),
        "acme.addon",
        "[requires]\nextensions = { \"acme.app\" = \"*\" }",
    );
    let fine = package(
        temp.path(),
        "acme.fine",
        "[requires]\nextensions = { \"acme.base\" = \"^1\" }",
    );
    let tool = package(
        temp.path(),
        "acme.tool",
        "[requires]\ntools = { \"node\" = \"*\" }",
    );
    let off = package(temp.path(), "acme.off", "");
    let mut state = StateFile::default();
    state.dev.extend([base, app, addon, fine, tool, off]);
    state.disabled.insert(ExtensionId::new("acme.off"));

    let registry = Registry::load(Some(&dirs), &state);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    let active: Vec<&str> = registry.active().map(|l| l.def.id.as_str()).collect();
    assert_eq!(active, ["acme.base", "acme.fine"]);
    let blocked = |id: &str| registry.blocked(&ExtensionId::new(id)).cloned();
    assert_eq!(blocked("acme.off"), Some(Blocked::Disabled));
    let Some(Blocked::Missing(app)) = blocked("acme.app") else {
        panic!()
    };
    assert_eq!(app[0].reason, "1.0.0 is installed");
    let Some(Blocked::Missing(addon)) = blocked("acme.addon") else {
        panic!()
    };
    assert!(addon[0].reason.contains("not in use"), "{addon:?}");
    let Some(Blocked::Missing(tool)) = blocked("acme.tool") else {
        panic!()
    };
    assert_eq!(tool[0].kind, RequireKind::Tool);

    let bad = problems(
        "acme.bad",
        "[requires]\nextensions = { \"acme.bad\" = \"*\", \"acme.base\" = \"not a range\" }",
    );
    assert_eq!(bad.len(), 2, "{bad:?}");
}

#[test]
fn state_files_round_trip_and_bad_ones_are_set_aside() {
    let temp = tempfile::tempdir().unwrap();
    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
    let path = dirs.state_file();
    let mut state = StateFile::default();
    state.disabled.insert(ExtensionId::new("acme.x"));
    state.dev.insert(temp.path().join("dev"));
    state.save(&path).unwrap();
    assert_eq!(StateFile::load(&path).unwrap(), Some(state));

    fs::write(&path, "not json").unwrap();
    let (fresh, err) = StateFile::load_or_default(&path);
    assert_eq!(fresh, StateFile::default());
    assert!(err.is_some());
    assert!(path.with_extension("json.bad").exists());
}

#[test]
fn this_build_has_a_download_target() {
    if cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )) {
        assert!(current_target().is_some());
    }
}
