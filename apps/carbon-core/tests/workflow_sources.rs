//! Phase 4, rest of install: git sources, content hashes, the catalog and
//! `[requires]`. Git repositories are made with the git CLI in a temporary
//! folder and fetched over the local transport (no network).

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;

use carbon_core::workflow::{
    Catalog, InstallKind, InstallSource, Registry, RequireKind, WorkflowDirs, WorkflowId,
    fetch_catalog, fetch_from_catalog, fetch_git, plan_install, read_package,
};

const KINDS: [&str; 5] = ["editor", "explorer", "terminal", "chat", "placeholder"];

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
panels = ["main"]

[panels]
main = {{ kind = "placeholder", title = "Main" }}
"#
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Runs git in `dir` and returns its trimmed output.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A repository whose first commit (tagged `v1.0.0`) holds version 1.0.0
/// of `acme.git` and whose second commit (on `main`) holds 2.0.0. Returns
/// (URL, first commit id).
fn package_repo(dir: &Path) -> (String, String) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    write(
        &dir.join("workflow.toml"),
        &manifest("acme.git", "1.0.0", ""),
    );
    write(&dir.join("assets/readme.txt"), "one");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "one"]);
    git(dir, &["tag", "v1.0.0"]);
    let first = git(dir, &["rev-parse", "HEAD"]);
    write(
        &dir.join("workflow.toml"),
        &manifest("acme.git", "2.0.0", ""),
    );
    write(&dir.join("assets/readme.txt"), "two");
    git(dir, &["commit", "-q", "-am", "two"]);
    (url(dir), first)
}

fn url(dir: &Path) -> String {
    dir.display().to_string().replace('\\', "/")
}

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn git_sources_fetch_a_tag_a_branch_or_a_commit() {
    let temp = tempfile::tempdir().unwrap();
    let (url, first) = package_repo(&temp.path().join("repo"));

    let tag = fetch_git(&url, "v1.0.0", &never()).unwrap();
    assert_eq!(tag.def.version, "1.0.0");
    assert_eq!(tag.files, ["assets/readme.txt", "workflow.toml"]);
    let record = tag.record();
    assert_eq!(record.rev.as_deref(), Some("v1.0.0"));
    assert_eq!(record.commit.as_deref(), Some(first.as_str()));
    assert!(!record.pinned, "a tag can move");

    let head = fetch_git(&url, "", &never()).unwrap();
    assert_eq!(head.def.version, "2.0.0");
    let branch = fetch_git(&url, "main", &never()).unwrap();
    assert_eq!(branch.def.version, "2.0.0");

    let commit = fetch_git(&url, &first, &never()).unwrap();
    assert_eq!(commit.def.version, "1.0.0");
    let record = commit.record();
    assert!(record.pinned && record.rev.is_none());
    assert_eq!(commit.hash, tag.hash);

    // Installs like any package; the temporary checkout is enough.
    let dirs = WorkflowDirs::new(temp.path().join("prefs"));
    let manifest = dirs.install(&commit).unwrap();
    let readme = manifest.parent().unwrap().join("assets/readme.txt");
    assert_eq!(fs::read_to_string(readme).unwrap(), "one");

    assert!(fetch_git(&url, "no-such-tag", &never()).is_err());
    let unknown = "0123456789abcdef0123456789abcdef01234567";
    assert!(fetch_git(&url, unknown, &never()).is_err());
}

#[test]
fn git_sources_refuse_links() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("repo");
    fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    write(
        &dir.join("workflow.toml"),
        &manifest("acme.link", "1.0.0", ""),
    );
    git(&dir, &["add", "-A"]);
    // A symbolic link entry, made without touching the file system.
    let target = dir.join("target.txt");
    write(&target, "../../outside");
    let blob = git(&dir, &["hash-object", "-w", "target.txt"]);
    fs::remove_file(target).unwrap();
    let cacheinfo = format!("120000,{blob},escape");
    git(&dir, &["update-index", "--add", "--cacheinfo", &cacheinfo]);
    git(&dir, &["commit", "-q", "-m", "link"]);

    let problems = fetch_git(&url(&dir), "", &never()).unwrap_err();
    assert!(
        problems.iter().any(|problem| problem.contains("links")),
        "{problems:?}"
    );
}

#[test]
fn the_content_hash_is_the_same_from_a_folder_or_a_zip() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("pkg");
    let text = manifest("acme.hash", "1.0.0", "");
    write(&folder.join("workflow.toml"), &text);
    write(&folder.join("a/b.txt"), "bytes");
    let zip_path = temp.path().join("pkg.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    // Not in sorted order, inside a top folder: the hash does not care.
    for (name, body) in [
        ("top/a/b.txt", "bytes"),
        ("top/workflow.toml", text.as_str()),
    ] {
        zip.start_file(name, options).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();

    let from_folder = read_package(InstallSource::from_path(&folder)).unwrap();
    let from_zip = read_package(InstallSource::from_path(&zip_path)).unwrap();
    assert!(from_folder.hash.starts_with("sha256:") && from_folder.hash.len() == 71);
    assert_eq!(from_folder.hash, from_zip.hash);

    write(&folder.join("a/b.txt"), "changed");
    let changed = read_package(InstallSource::from_path(&folder)).unwrap();
    assert_ne!(changed.hash, from_folder.hash);
    // The source changed after it was read: install refuses.
    let dirs = WorkflowDirs::new(temp.path().join("prefs"));
    let err = dirs.install(&from_folder).unwrap_err();
    assert!(err.contains("changed since it was read"), "{err}");
}

#[test]
fn the_catalog_keeps_good_entries_and_picks_versions() {
    let hash = format!("sha256:{}", "a".repeat(64));
    let commit = "b".repeat(40);
    let text = format!(
        r#"{{ "schema": 1, "packages": [
        {{ "id": "acme.one", "name": "One", "versions": [
            {{ "version": "1.0.0", "git": "x", "commit": "{commit}", "hash": "{hash}" }},
            {{ "version": "1.5.0", "git": "x", "commit": "{commit}", "hash": "{hash}" }},
            {{ "version": "2.0.0", "git": "x", "commit": "{commit}", "hash": "{hash}" }},
            {{ "version": "3.0.0", "git": "x", "commit": "short", "hash": "{hash}" }} ] }},
        {{ "id": "Bad Id", "name": "Bad", "versions": [] }},
        {{ "id": "acme.nohash", "name": "No hash", "versions": [
            {{ "version": "1.0.0", "git": "x", "commit": "{commit}", "hash": "md5:1" }} ] }}
    ] }}"#
    );
    let (catalog, problems) = Catalog::parse(&text).unwrap();
    assert_eq!(catalog.packages.len(), 1);
    assert_eq!(problems.len(), 3, "{problems:?}");
    let one = catalog.get(&WorkflowId::new("acme.one")).unwrap();
    assert_eq!(one.versions.len(), 3);
    assert_eq!(one.best(None).unwrap().version, "2.0.0");
    let range = semver::VersionReq::parse("^1.0").unwrap();
    assert_eq!(one.best(Some(&range)).unwrap().version, "1.5.0");

    assert!(Catalog::parse(r#"{ "schema": 2 }"#).is_err());
}

#[test]
fn catalog_installs_are_pinned_by_commit_and_hash() {
    let temp = tempfile::tempdir().unwrap();
    let (package_url, first) = package_repo(&temp.path().join("repo"));
    let good = fetch_git(&package_url, &first, &never()).unwrap().hash;

    let catalog_dir = temp.path().join("catalog");
    fs::create_dir_all(&catalog_dir).unwrap();
    git(&catalog_dir, &["init", "-q", "-b", "main"]);
    let entry = |version: &str, hash: &str| {
        format!(
            r#"{{ "version": "{version}", "git": "{package_url}", "commit": "{first}", "hash": "{hash}" }}"#
        )
    };
    let wrong = format!("sha256:{}", "0".repeat(64));
    write(
        &catalog_dir.join("catalog.json"),
        &format!(
            r#"{{ "schema": 1, "packages": [ {{ "id": "acme.git", "name": "Git", "versions": [ {}, {}, {} ] }} ] }}"#,
            entry("1.0.0", &good),
            entry("1.0.1", &good),
            entry("1.0.2", &wrong),
        ),
    );
    git(&catalog_dir, &["add", "-A"]);
    git(&catalog_dir, &["commit", "-q", "-m", "catalog"]);

    let text = fetch_catalog(&url(&catalog_dir), &never()).unwrap();
    let (catalog, problems) = Catalog::parse(&text).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    let listed = catalog.get(&WorkflowId::new("acme.git")).unwrap();
    let dirs = WorkflowDirs::new(temp.path().join("prefs"));
    let registry = Registry::load(Some(&dirs), &KINDS);
    let plan_for = |version: &str| {
        let entry = listed
            .versions
            .iter()
            .find(|v| v.version == version)
            .unwrap();
        let package = fetch_from_catalog(listed, entry, &never()).unwrap();
        (plan_install(&package, &registry, &BTreeSet::new()), package)
    };

    let (plan, package) = plan_for("1.0.0");
    assert!(plan.problems.is_empty(), "{:?}", plan.problems);
    assert_eq!(plan.kind, InstallKind::New);
    assert!(package.record().pinned);
    // The commit holds 1.0.0, not the 1.0.1 the entry claims.
    let (plan, _) = plan_for("1.0.1");
    assert!(plan.problems[0].contains("version"), "{:?}", plan.problems);
    let (plan, _) = plan_for("1.0.2");
    assert!(plan.problems.iter().any(|p| p.contains("content hash")));
}

#[test]
fn missing_requirements_keep_a_workflow_off() {
    let base = manifest("acme.base", "1.2.0", "");
    let needs_new_base = manifest(
        "acme.app",
        "1.0.0",
        "[requires]\nworkflows = { \"acme.base\" = \">=2.0\" }",
    );
    let needs_app = manifest(
        "acme.addon",
        "1.0.0",
        "[requires]\nworkflows = { \"acme.app\" = \"*\" }",
    );
    let fine = manifest(
        "acme.fine",
        "1.0.0",
        "[requires]\nworkflows = { \"acme.base\" = \"^1\" }",
    );
    let extension = manifest(
        "acme.ext",
        "1.0.0",
        "[requires]\nextensions = { \"rust-analyzer\" = \"*\" }",
    );
    let registry = Registry::from_sources(
        &[
            ("base", &base),
            ("app", &needs_new_base),
            ("addon", &needs_app),
            ("fine", &fine),
            ("ext", &extension),
        ],
        &KINDS,
        false,
    );
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    let available: Vec<&str> = registry.available().map(|def| def.id.as_str()).collect();
    assert_eq!(available, ["acme.base", "acme.fine"]);

    let app = registry.missing(&WorkflowId::new("acme.app"));
    assert_eq!(app.len(), 1);
    assert_eq!(app[0].reason, "1.2.0 is installed");
    let addon = registry.missing(&WorkflowId::new("acme.addon"));
    assert!(addon[0].reason.contains("cannot run"), "{addon:?}");
    let ext = registry.missing(&WorkflowId::new("acme.ext"));
    assert_eq!(ext[0].kind, RequireKind::Extension);
    assert!(registry.missing(&WorkflowId::new("acme.fine")).is_empty());

    let bad = manifest(
        "acme.bad",
        "1.0.0",
        "[requires]\nworkflows = { \"acme.bad\" = \"*\", \"acme.base\" = \"not a range\" }",
    );
    let registry = Registry::from_sources(&[("bad", &bad)], &KINDS, false);
    let problems = &registry.rejected()[0].problems;
    assert_eq!(problems.len(), 2, "{problems:?}");
}

/// Real HTTPS through gix + reqwest + rustls: a small public repository with
/// no `workflow.toml`, so the fetch works and the package check fails.
#[test]
#[ignore = "needs the network"]
fn https_sources_fetch() {
    let problems = fetch_git(
        "https://github.com/octocat/Hello-World.git",
        "master",
        &never(),
    )
    .unwrap_err();
    assert!(problems[0].contains("no workflow.toml"), "{problems:?}");

    // The cause is in the message, not only gix's "Transport handshake failed".
    let problems = fetch_git("https://example.invalid/acme/x.git", "", &never()).unwrap_err();
    println!("{problems:?}");
    assert!(problems[0].matches(": ").count() >= 2, "{problems:?}");
}
