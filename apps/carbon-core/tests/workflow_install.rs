//! Phase 4: reading, planning and installing workflow packages; capabilities.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use carbon_core::workflow::{
    InstallKind, InstallSource, Origin, Registry, WorkflowDirs, WorkflowId, plan_install,
    read_package,
};

const KINDS: [&str; 7] = [
    "editor",
    "explorer",
    "terminal",
    "chat",
    "placeholder",
    "view",
    "webview",
];

fn manifest(id: &str, version: &str, extra: &str) -> String {
    format!(
        r#"
id = "{id}"
schema_version = 1
version = "{version}"
carbon_api = ">=0.1, <0.2"
name = "Pkg"
{extra}

[layout.center]
panels = ["web"]

[panels]
web = {{ kind = "webview", page = "ui/index.html" }}
"#
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A package folder with a manifest and its page.
fn package_folder(dir: &Path, id: &str, version: &str, extra: &str) {
    write(&dir.join("workflow.toml"), &manifest(id, version, extra));
    write(&dir.join("ui/index.html"), "<p>page</p>");
}

/// A zip with `entries` (name, text), stored uncompressed.
fn zip_file(path: &Path, entries: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, text) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn setup() -> (tempfile::TempDir, WorkflowDirs) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = WorkflowDirs::new(temp.path().join("prefs"));
    (temp, dirs)
}

#[test]
fn a_folder_installs_and_loads() {
    let (temp, dirs) = setup();
    let source = temp.path().join("src");
    package_folder(&source, "acme.web", "1.0.0", "");
    write(&source.join(".git/config"), "left out");

    let package = read_package(InstallSource::from_path(&source)).unwrap();
    assert_eq!(package.files, ["ui/index.html", "workflow.toml"]);
    let plan = plan_install(
        &package,
        &Registry::load(Some(&dirs), &KINDS),
        &BTreeSet::new(),
    );
    assert_eq!(plan.kind, InstallKind::New);
    assert!(plan.problems.is_empty());

    let manifest = dirs.install(&package).unwrap();
    assert!(manifest.ends_with("installed/acme.web/1.0.0/workflow.toml"));
    let registry = Registry::load(Some(&dirs), &KINDS);
    let id = WorkflowId::new("acme.web");
    assert!(matches!(registry.origin(&id), Some(Origin::Installed(_))));
    let page = registry.package_dir(&id).unwrap().join("ui/index.html");
    assert_eq!(fs::read_to_string(page).unwrap(), "<p>page</p>");
}

#[test]
fn a_zip_installs_with_or_without_a_top_folder() {
    let (temp, dirs) = setup();
    let text = manifest("acme.zip", "1.0.0", "");
    let flat = temp.path().join("flat.zip");
    zip_file(
        &flat,
        &[("workflow.toml", &text), ("ui/index.html", "flat")],
    );
    let wrapped = temp.path().join("wrapped.zip");
    zip_file(
        &wrapped,
        &[
            ("pkg/workflow.toml", &text),
            ("pkg/ui/index.html", "wrapped"),
        ],
    );

    // Windows PowerShell's Compress-Archive writes `\` separators.
    let backslash = temp.path().join("backslash.zip");
    zip_file(
        &backslash,
        &[("workflow.toml", &text), ("ui\\index.html", "backslash")],
    );

    for (zip, body) in [
        (&flat, "flat"),
        (&wrapped, "wrapped"),
        (&backslash, "backslash"),
    ] {
        let package = read_package(InstallSource::from_path(zip)).unwrap();
        assert_eq!(package.files, ["ui/index.html", "workflow.toml"]);
        let manifest = dirs.install(&package).unwrap();
        let page = manifest.parent().unwrap().join("ui/index.html");
        assert_eq!(fs::read_to_string(page).unwrap(), body);
    }
}

#[test]
fn bad_packages_are_refused_before_anything_is_written() {
    let (temp, dirs) = setup();
    let text = manifest("acme.bad", "1.0.0", "");
    let slip = temp.path().join("slip.zip");
    zip_file(
        &slip,
        &[
            ("workflow.toml", &text),
            ("ui/index.html", "x"),
            ("../evil.txt", "x"),
        ],
    );
    assert!(read_package(InstallSource::from_path(&slip)).is_err());
    let back_slip = temp.path().join("backslip.zip");
    zip_file(
        &back_slip,
        &[
            ("workflow.toml", &text),
            ("ui/index.html", "x"),
            ("..\\evil.txt", "x"),
        ],
    );
    assert!(read_package(InstallSource::from_path(&back_slip)).is_err());

    let no_manifest = temp.path().join("empty.zip");
    zip_file(&no_manifest, &[("readme.txt", "x")]);
    assert!(read_package(InstallSource::from_path(&no_manifest)).is_err());

    let missing_page = temp.path().join("nopage");
    write(&missing_page.join("workflow.toml"), &text);
    let problems = read_package(InstallSource::from_path(&missing_page)).unwrap_err();
    assert!(problems[0].contains("not in the package"), "{problems:?}");

    let bad_capability = temp.path().join("badcap");
    package_folder(
        &bad_capability,
        "acme.cap",
        "1.0.0",
        r#"capabilities = ["teleport"]"#,
    );
    assert!(read_package(InstallSource::from_path(&bad_capability)).is_err());

    let reserved = temp.path().join("reserved");
    package_folder(&reserved, "carbon.fake", "1.0.0", "");
    assert!(read_package(InstallSource::from_path(&reserved)).is_err());

    assert!(!dirs.installed_dir().exists());
}

#[test]
fn updates_downgrades_and_capabilities_are_planned() {
    let (temp, dirs) = setup();
    let v1 = temp.path().join("v1");
    package_folder(
        &v1,
        "acme.cap",
        "1.0.0",
        r#"capabilities = ["gpu", "net:api.example.com"]"#,
    );
    dirs.install(&read_package(InstallSource::from_path(&v1)).unwrap())
        .unwrap();

    let v2 = temp.path().join("v2");
    package_folder(
        &v2,
        "acme.cap",
        "2.0.0",
        r#"capabilities = ["gpu", "midi"]"#,
    );
    let package = read_package(InstallSource::from_path(&v2)).unwrap();
    let granted: BTreeSet<String> = ["gpu", "net:api.example.com"].map(String::from).into();
    let plan = plan_install(&package, &Registry::load(Some(&dirs), &KINDS), &granted);
    assert_eq!(
        plan.kind,
        InstallKind::Update {
            from: "1.0.0".into()
        }
    );
    assert_eq!(plan.new_capabilities, ["midi"]);
    assert_eq!(plan.kept_capabilities, ["gpu"]);
    assert_eq!(plan.dropped_capabilities, ["net:api.example.com"]);
    assert!(!plan.layout_changed);

    // Installing 2.0.0 removes 1.0.0; installing 1.0.0 again is a downgrade that sticks.
    dirs.install(&package).unwrap();
    let id_dir = dirs.installed_dir().join("acme.cap");
    assert!(!id_dir.join("1.0.0").exists() && id_dir.join("2.0.0").exists());
    let old = read_package(InstallSource::from_path(&v1)).unwrap();
    let plan = plan_install(&old, &Registry::load(Some(&dirs), &KINDS), &granted);
    assert_eq!(
        plan.kind,
        InstallKind::Downgrade {
            from: "2.0.0".into()
        }
    );
    dirs.install(&old).unwrap();
    let registry = Registry::load(Some(&dirs), &KINDS);
    assert_eq!(
        registry.get(&WorkflowId::new("acme.cap")).unwrap().version,
        "1.0.0"
    );
}

#[test]
fn a_user_workflow_id_cannot_be_taken() {
    let (temp, dirs) = setup();
    package_folder(&dirs.user_dir().join("mine"), "acme.same", "1.0.0", "");
    let source = temp.path().join("pkg");
    package_folder(&source, "acme.same", "1.0.0", "");
    let package = read_package(InstallSource::from_path(&source)).unwrap();
    let plan = plan_install(
        &package,
        &Registry::load(Some(&dirs), &KINDS),
        &BTreeSet::new(),
    );
    assert!(
        plan.problems[0].contains("user workflow"),
        "{:?}",
        plan.problems
    );
}

#[test]
fn duplicate_copies_package_files() {
    let (temp, dirs) = setup();
    let source = temp.path().join("src");
    package_folder(&source, "acme.dup", "1.0.0", "");
    dirs.install(&read_package(InstallSource::from_path(&source)).unwrap())
        .unwrap();
    let registry = Registry::load(Some(&dirs), &KINDS);
    let id = WorkflowId::new("acme.dup");
    let copy = registry
        .get(&id)
        .unwrap()
        .duplicate(WorkflowId::new("user.dup"), "Copy");
    let manifest = dirs
        .create_user_workflow(&copy, registry.package_dir(&id))
        .unwrap();
    let page = manifest.parent().unwrap().join("ui/index.html");
    assert_eq!(fs::read_to_string(page).unwrap(), "<p>page</p>");
}
