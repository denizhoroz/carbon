//! Extension packages: path rules, reading and installing from a folder, a
//! zip or git, content hashes, plans, the catalog and its signatures. Ported
//! from the shelved workflow branch. Git repositories are made with the git
//! CLI in a temporary folder and fetched over the local transport (no network).

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use carbon_core::extension::{
    Catalog, CatalogConfig, CatalogFiles, ExtensionDirs, ExtensionId, FetchControl, InstallKind,
    InstallSource, MAX_PACKAGE_PATH, Origin, Pin, Registry, Signing, StateFile, check_package_path,
    fetch_catalog, fetch_from_catalog, fetch_git, installed_hash, plan_install, read_package,
    resolve_package_file, verify_installed,
};

fn manifest(id: &str, version: &str, extra: &str) -> String {
    format!(
        r#"
id = "{id}"
schema_version = 1
version = "{version}"
carbon_api = ">=0.1, <0.2"
name = "Pkg"
{extra}

[[themes]]
file = "themes/pkg.json"
"#
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A package folder with a manifest and the theme file it names.
fn package_folder(dir: &Path, id: &str, version: &str, extra: &str) {
    write(&dir.join("extension.toml"), &manifest(id, version, extra));
    write(&dir.join("themes/pkg.json"), "{}");
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

fn setup() -> (tempfile::TempDir, ExtensionDirs) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
    (temp, dirs)
}

fn load(dirs: &ExtensionDirs) -> Registry {
    Registry::load(Some(dirs), &StateFile::default())
}

fn never() -> FetchControl {
    FetchControl::default()
}

#[test]
fn package_paths_follow_the_rules() {
    for good in [
        "index.html",
        "ui/index.html",
        "assets/app.v2.js",
        "a b/c-d_e.css",
    ] {
        assert!(check_package_path(good).is_ok(), "{good} should pass");
    }
    let long = "a/".repeat(MAX_PACKAGE_PATH) + "x";
    for bad in [
        "",
        "/abs.html",
        "../outside.html",
        "ui/../index.html",
        "./index.html",
        "ui//index.html",
        "ui\\index.html",
        "C:/index.html",
        "con.html",
        "ui/NUL",
        "what?.html",
        "trailing.",
        long.as_str(),
    ] {
        assert!(check_package_path(bad).is_err(), "{bad:?} should fail");
    }
}

#[test]
fn files_resolve_inside_the_package_with_exact_case() {
    let temp = tempfile::tempdir().unwrap();
    write(&temp.path().join("ui/Index.html"), "<p>hi</p>");
    let file = resolve_package_file(temp.path(), "ui/Index.html").unwrap();
    assert_eq!(fs::read_to_string(file).unwrap(), "<p>hi</p>");
    // Windows would open these; a case-sensitive system would not.
    assert!(resolve_package_file(temp.path(), "ui/index.html").is_err());
    assert!(resolve_package_file(temp.path(), "UI/Index.html").is_err());
    assert!(resolve_package_file(temp.path(), "ui/missing.html").is_err());
    assert!(resolve_package_file(temp.path(), "ui").is_err());
    assert!(resolve_package_file(&temp.path().join("ui"), "../ui/Index.html").is_err());
}

#[test]
fn a_folder_installs_and_loads() {
    let (temp, dirs) = setup();
    let source = temp.path().join("src");
    package_folder(&source, "acme.theme", "1.0.0", "");
    write(&source.join(".git/config"), "left out");

    let package = read_package(InstallSource::from_path(&source)).unwrap();
    assert_eq!(package.files, ["extension.toml", "themes/pkg.json"]);
    let plan = plan_install(&package, &load(&dirs), &BTreeSet::new());
    assert_eq!(plan.kind, InstallKind::New);
    assert!(plan.problems.is_empty(), "{:?}", plan.problems);

    let manifest = dirs.install(&package).unwrap();
    assert!(manifest.ends_with("installed/acme.theme/1.0.0/extension.toml"));
    let registry = load(&dirs);
    let loaded = registry.get(&ExtensionId::new("acme.theme")).unwrap();
    assert_eq!(loaded.origin, Origin::Installed);
    assert!(registry.is_active(&loaded.def.id));
    let theme = loaded.file("themes/pkg.json").unwrap();
    assert_eq!(fs::read_to_string(theme).unwrap(), "{}");
}

#[test]
fn a_zip_installs_with_or_without_a_top_folder() {
    let (temp, dirs) = setup();
    let text = manifest("acme.zip", "1.0.0", "");
    let flat = temp.path().join("flat.zip");
    zip_file(
        &flat,
        &[("extension.toml", &text), ("themes/pkg.json", "1")],
    );
    let wrapped = temp.path().join("wrapped.zip");
    zip_file(
        &wrapped,
        &[("pkg/extension.toml", &text), ("pkg/themes/pkg.json", "2")],
    );
    // Windows PowerShell's Compress-Archive writes `\` separators.
    let backslash = temp.path().join("backslash.zip");
    zip_file(
        &backslash,
        &[("extension.toml", &text), ("themes\\pkg.json", "3")],
    );
    for (zip, body) in [(&flat, "1"), (&wrapped, "2"), (&backslash, "3")] {
        let package = read_package(InstallSource::from_path(zip)).unwrap();
        assert_eq!(package.files, ["extension.toml", "themes/pkg.json"]);
        let manifest = dirs.install(&package).unwrap();
        let theme = manifest.parent().unwrap().join("themes/pkg.json");
        assert_eq!(fs::read_to_string(theme).unwrap(), body);
    }
}

#[test]
fn bad_packages_are_refused_before_anything_is_written() {
    let (temp, dirs) = setup();
    let text = manifest("acme.bad", "1.0.0", "");
    for name in ["../evil.txt", "..\\evil.txt"] {
        let slip = temp.path().join("slip.zip");
        zip_file(
            &slip,
            &[
                ("extension.toml", &text),
                ("themes/pkg.json", "x"),
                (name, "x"),
            ],
        );
        assert!(
            read_package(InstallSource::from_path(&slip)).is_err(),
            "{name}"
        );
    }
    let no_manifest = temp.path().join("empty.zip");
    zip_file(&no_manifest, &[("readme.txt", "x")]);
    assert!(read_package(InstallSource::from_path(&no_manifest)).is_err());

    let missing_file = temp.path().join("nofile");
    write(&missing_file.join("extension.toml"), &text);
    let problems = read_package(InstallSource::from_path(&missing_file)).unwrap_err();
    assert!(problems[0].contains("not in the package"), "{problems:?}");

    let bad_capability = temp.path().join("badcap");
    package_folder(
        &bad_capability,
        "acme.cap",
        "1.0.0",
        r#"capabilities = ["teleport"]"#,
    );
    assert!(read_package(InstallSource::from_path(&bad_capability)).is_err());

    assert!(!dirs.installed_dir().exists());
}

#[test]
fn official_ids_need_a_signed_catalog() {
    let (temp, dirs) = setup();
    let source = temp.path().join("official");
    package_folder(&source, "carbon.fake", "1.0.0", "");
    let mut package = read_package(InstallSource::from_path(&source)).unwrap();
    let plan = plan_install(&package, &load(&dirs), &BTreeSet::new());
    assert!(plan.problems[0].contains("signed"), "{:?}", plan.problems);

    package.pin = Some(Pin {
        id: ExtensionId::new("carbon.fake"),
        version: "1.0.0".into(),
        hash: package.hash.clone(),
        signed: true,
    });
    assert!(
        plan_install(&package, &load(&dirs), &BTreeSet::new())
            .problems
            .is_empty()
    );

    // Installed without a signed record (copied in by hand): not loaded.
    dirs.install(&package).unwrap();
    let registry = load(&dirs);
    assert!(registry.get(&ExtensionId::new("carbon.fake")).is_none());
    assert!(registry.rejected()[0].problems[0].contains("signed catalog"));
    let mut state = StateFile::default();
    state
        .installs
        .insert(ExtensionId::new("carbon.fake"), package.record());
    let registry = Registry::load(Some(&dirs), &state);
    assert!(registry.is_active(&ExtensionId::new("carbon.fake")));
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
    let plan = plan_install(&package, &load(&dirs), &granted);
    assert_eq!(
        plan.kind,
        InstallKind::Update {
            from: "1.0.0".into()
        }
    );
    assert_eq!(plan.new_capabilities, ["midi"]);
    assert_eq!(plan.kept_capabilities, ["gpu"]);
    assert_eq!(plan.dropped_capabilities, ["net:api.example.com"]);

    // Installing 2.0.0 removes 1.0.0; installing 1.0.0 again is a downgrade that sticks.
    dirs.install(&package).unwrap();
    let id_dir = dirs.installed_dir().join("acme.cap");
    assert!(!id_dir.join("1.0.0").exists() && id_dir.join("2.0.0").exists());
    let old = read_package(InstallSource::from_path(&v1)).unwrap();
    let plan = plan_install(&old, &load(&dirs), &granted);
    assert_eq!(
        plan.kind,
        InstallKind::Downgrade {
            from: "2.0.0".into()
        }
    );
    dirs.install(&old).unwrap();
    let registry = load(&dirs);
    assert_eq!(
        registry
            .get(&ExtensionId::new("acme.cap"))
            .unwrap()
            .def
            .version,
        "1.0.0"
    );
}

#[test]
fn a_linked_dev_folder_wins_and_blocks_installing_its_id() {
    let (temp, dirs) = setup();
    let installed = temp.path().join("pkg");
    package_folder(&installed, "acme.same", "1.0.0", "");
    let package = read_package(InstallSource::from_path(&installed)).unwrap();
    dirs.install(&package).unwrap();
    let dev = temp.path().join("dev");
    package_folder(&dev, "acme.same", "9.0.0", "");
    let mut state = StateFile::default();
    state.dev.insert(dev.clone());

    let registry = Registry::load(Some(&dirs), &state);
    let loaded = registry.get(&ExtensionId::new("acme.same")).unwrap();
    assert_eq!(
        (loaded.origin.clone(), loaded.def.version.as_str()),
        (Origin::Dev, "9.0.0")
    );
    assert_eq!(loaded.dir, dev);
    assert!(registry.warnings()[0].1.contains("development"));
    let plan = plan_install(&package, &registry, &BTreeSet::new());
    assert!(plan.problems[0].contains("unlink"), "{:?}", plan.problems);
}

#[test]
fn removing_stays_inside_the_installed_folder() {
    let (temp, dirs) = setup();
    let source = temp.path().join("src");
    package_folder(&source, "acme.gone", "1.0.0", "");
    dirs.install(&read_package(InstallSource::from_path(&source)).unwrap())
        .unwrap();
    let registry = load(&dirs);
    let loaded = registry.get(&ExtensionId::new("acme.gone")).unwrap();
    dirs.remove(&loaded.origin, &loaded.dir).unwrap();
    assert!(!dirs.installed_dir().join("acme.gone").exists());
    assert!(load(&dirs).get(&ExtensionId::new("acme.gone")).is_none());
    // Development folders are never deleted; nor is anything outside.
    assert!(dirs.remove(&Origin::Dev, &source).is_err());
    assert!(
        dirs.remove(&Origin::Installed, &source.join("themes"))
            .is_err()
    );
    assert!(source.exists());
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
    package_folder(dir, "acme.git", "1.0.0", "");
    write(&dir.join("assets/readme.txt"), "one");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "one"]);
    git(dir, &["tag", "v1.0.0"]);
    let first = git(dir, &["rev-parse", "HEAD"]);
    write(
        &dir.join("extension.toml"),
        &manifest("acme.git", "2.0.0", ""),
    );
    write(&dir.join("assets/readme.txt"), "two");
    git(dir, &["commit", "-q", "-am", "two"]);
    (url(dir), first)
}

fn url(dir: &Path) -> String {
    dir.display().to_string().replace('\\', "/")
}

#[test]
fn git_sources_fetch_a_tag_a_branch_or_a_commit() {
    let temp = tempfile::tempdir().unwrap();
    let (url, first) = package_repo(&temp.path().join("repo"));

    let tag = fetch_git(&url, "v1.0.0", &never()).unwrap();
    assert_eq!(tag.def.version, "1.0.0");
    assert_eq!(
        tag.files,
        ["assets/readme.txt", "extension.toml", "themes/pkg.json"]
    );
    let record = tag.record();
    assert_eq!(record.rev.as_deref(), Some("v1.0.0"));
    assert_eq!(record.commit.as_deref(), Some(first.as_str()));
    assert!(!record.pinned, "a tag can move");

    assert_eq!(fetch_git(&url, "", &never()).unwrap().def.version, "2.0.0");
    assert_eq!(
        fetch_git(&url, "main", &never()).unwrap().def.version,
        "2.0.0"
    );

    let commit = fetch_git(&url, &first, &never()).unwrap();
    assert_eq!(commit.def.version, "1.0.0");
    let record = commit.record();
    assert!(record.pinned && record.rev.is_none());
    assert_eq!(commit.hash, tag.hash);

    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
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
    package_folder(&dir, "acme.link", "1.0.0", "");
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
    assert!(problems.iter().any(|p| p.contains("links")), "{problems:?}");
}

#[test]
fn the_content_hash_is_the_same_from_a_folder_or_a_zip() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("pkg");
    let text = manifest("acme.hash", "1.0.0", "");
    package_folder(&folder, "acme.hash", "1.0.0", "");
    let zip_path = temp.path().join("pkg.zip");
    // Not in sorted order, inside a top folder: the hash does not care.
    zip_file(
        &zip_path,
        &[("top/themes/pkg.json", "{}"), ("top/extension.toml", &text)],
    );
    let from_folder = read_package(InstallSource::from_path(&folder)).unwrap();
    let from_zip = read_package(InstallSource::from_path(&zip_path)).unwrap();
    assert!(from_folder.hash.starts_with("sha256:") && from_folder.hash.len() == 71);
    assert_eq!(from_folder.hash, from_zip.hash);

    write(&folder.join("themes/pkg.json"), "changed");
    // The source changed after it was read: install refuses.
    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
    let err = dirs.install(&from_folder).unwrap_err();
    assert!(err.contains("changed since it was read"), "{err}");
}

#[test]
fn the_catalog_keeps_good_entries_and_picks_versions() {
    let hash = format!("sha256:{}", "a".repeat(64));
    let commit = "b".repeat(40);
    let text = format!(
        r#"{{ "schema": 1, "packages": [
        {{ "id": "acme.one", "name": "One", "suffixes": ["one"], "versions": [
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
    let one = catalog.get(&ExtensionId::new("acme.one")).unwrap();
    assert_eq!(one.suffixes, ["one"]);
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

    let files = fetch_catalog(&url(&catalog_dir), &never()).unwrap();
    assert!(files.signature.is_none());
    let (catalog, problems) = Catalog::load(&files, &[]).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    let listed = catalog.get(&ExtensionId::new("acme.git")).unwrap();
    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
    let registry = load(&dirs);
    let plan_for = |version: &str| {
        let entry = listed
            .versions
            .iter()
            .find(|v| v.version == version)
            .unwrap();
        let package = fetch_from_catalog(listed, entry, false, &never()).unwrap();
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

/// Real HTTPS through gix + reqwest + rustls: a small public repository with
/// no `extension.toml`, so the fetch works and the package check fails.
#[test]
#[ignore = "needs the network"]
fn https_sources_fetch() {
    let problems = fetch_git(
        "https://github.com/octocat/Hello-World.git",
        "master",
        &never(),
    )
    .unwrap_err();
    assert!(problems[0].contains("no extension.toml"), "{problems:?}");
}

// Signed with a pure-Python minisign signer (seed 0x01 x 32, key number
// 1122334455667788); KEY_B is another key.
const SIGNED_TEXT: &str = r#"{ "schema": 1, "packages": [ { "id": "acme.signed", "name": "Signed", "versions": [ { "version": "1.0.0", "git": "x", "commit": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "hash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" } ] } ] }"#;
const KEY_A: &str = "RWQRIjNEVWZ3iIqI4910CfGV/VLbLTy6XXLKZwm/HZQSG/N0iAG0D29c";
const KEY_B: &str = "RWSId2ZVRDMiEYE5dw6ofRdfVqNUZsNMfszLjYqRtO43ol32D1uPybOU";
const SIGNATURE_A: &str = "untrusted comment: signature from minisign secret key
RUQRIjNEVWZ3iKzqCAH+ZONIWhQ9dObQEoXalpKYir3ZjDqpJJyRiQBVpHodM1slVixx1JczbjuCzfj/HzEE169RVoFl8KLoDQc=
trusted comment: timestamp:0\tfile:catalog.json\tacme test key
yixUw619twESshvoMqhlLclcBikWS1IbPvUUKZJO1yk4kYUaBWqPSOe+JiyTiOR0sJ/JTmS47m0z9ejxoHsXBw==
";

#[test]
fn signed_catalogs_need_a_trusted_key() {
    let files = CatalogFiles {
        text: SIGNED_TEXT.into(),
        signature: Some(SIGNATURE_A.into()),
    };
    let keys = |keys: &[&str]| keys.iter().map(|key| key.to_string()).collect::<Vec<_>>();
    // A whole `.pub` file works as a key too.
    let pub_file = format!("untrusted comment: minisign public key\n{KEY_A}\n");
    let (catalog, _) = Catalog::load(&files, &keys(&[KEY_B, &pub_file])).unwrap();
    assert!(catalog.is_signed());
    match catalog.signing.unwrap() {
        Signing::Trusted { key, comment } => {
            assert!(KEY_A.starts_with(&key));
            assert!(comment.contains("acme test key"));
        }
        other => panic!("{other:?}"),
    }
    let err = Catalog::load(&files, &keys(&[KEY_B])).unwrap_err();
    assert!(err.contains("not signed by a trusted key"), "{err}");
    let tampered = CatalogFiles {
        text: SIGNED_TEXT.replace("1.0.0", "1.0.1"),
        ..files.clone()
    };
    assert!(Catalog::load(&tampered, &keys(&[KEY_A])).is_err());
    let unsigned = CatalogFiles {
        signature: None,
        ..files.clone()
    };
    assert!(
        Catalog::load(&unsigned, &keys(&[KEY_A]))
            .unwrap_err()
            .contains("required")
    );
    // No keys: used, not checked.
    let (catalog, _) = Catalog::load(&unsigned, &[]).unwrap();
    assert_eq!(catalog.signing, Some(Signing::NotChecked));
    assert!(!catalog.is_signed());
}

#[test]
fn the_catalog_config_round_trips_and_checks_keys() {
    let temp = tempfile::tempdir().unwrap();
    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
    let path = dirs.catalog_config_file();
    assert_eq!(
        CatalogConfig::load(&path).unwrap(),
        CatalogConfig::default()
    );
    let config = CatalogConfig {
        url: Some("https://example.com/catalog.git".into()),
        keys: vec![KEY_A.into(), "not a key".into()],
    };
    config.save(&path).unwrap();
    assert_eq!(CatalogConfig::load(&path).unwrap(), config);
    let problems = config.check();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].starts_with("trusted key 2"), "{problems:?}");
}

#[test]
fn changed_installed_files_are_found() {
    let temp = tempfile::tempdir().unwrap();
    let (url, first) = package_repo(&temp.path().join("repo"));
    let package = fetch_git(&url, &first, &never()).unwrap();
    let dirs = ExtensionDirs::new(temp.path().join("prefs"));
    let manifest = dirs.install(&package).unwrap();
    let dir = manifest.parent().unwrap();
    verify_installed(dir, &package.hash).unwrap();

    write(&dir.join("assets/readme.txt"), "changed");
    assert!(verify_installed(dir, &package.hash).is_err());
    let now = installed_hash(dir).unwrap();
    verify_installed(dir, &now).unwrap();
    write(&dir.join("extra.txt"), "x");
    assert!(verify_installed(dir, &now).is_err());

    // A changed install is turned off until Reinstall or Keep Changes.
    let id = ExtensionId::new("acme.git");
    let mut state = StateFile::default();
    state.changed.insert(id.clone(), "files changed".into());
    let registry = Registry::load(Some(&dirs), &state);
    assert!(registry.get(&id).is_some() && !registry.is_active(&id));

    // Reinstalling the recorded commit with the recorded hash as its pin
    // restores the files.
    let mut again = fetch_git(&url, &first, &never()).unwrap();
    again.pin = Some(Pin {
        id: id.clone(),
        version: "1.0.0".into(),
        hash: package.hash.clone(),
        signed: false,
    });
    let plan = plan_install(&again, &load(&dirs), &BTreeSet::new());
    assert_eq!(plan.kind, InstallKind::Reinstall);
    assert!(plan.problems.is_empty(), "{:?}", plan.problems);
    let manifest = dirs.install(&again).unwrap();
    verify_installed(manifest.parent().unwrap(), &package.hash).unwrap();
}

#[test]
fn a_package_in_a_repository_folder_installs_alone() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("monorepo");
    fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    write(&dir.join("README.md"), "the app");
    package_folder(&dir.join("extensions/theme"), "acme.mono", "1.0.0", "");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    let commit = git(&dir, &["rev-parse", "HEAD"]);

    let package = carbon_core::extension::fetch_git_path(
        &url(&dir),
        &commit,
        Some("extensions/theme"),
        &never(),
    )
    .unwrap();
    assert_eq!(package.files, ["extension.toml", "themes/pkg.json"]);
    let record = package.record();
    assert_eq!(record.path.as_deref(), Some("extensions/theme"));
    assert!(record.pinned);
    let problems = carbon_core::extension::fetch_git_path(
        &url(&dir),
        &commit,
        Some("extensions/none"),
        &never(),
    )
    .unwrap_err();
    assert!(problems[0].contains("no folder"), "{problems:?}");
    assert!(
        carbon_core::extension::fetch_git_path(&url(&dir), &commit, Some("../x"), &never())
            .is_err()
    );
}
