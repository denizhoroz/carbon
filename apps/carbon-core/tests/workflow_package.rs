//! Package files (workflow-system.md Q22): path rules, resolving a page
//! inside its package, `page` on panel rules, package folders.

use std::fs;
use std::path::Path;

use carbon_core::workflow::{
    MAX_PACKAGE_PATH, Registry, WorkflowDef, WorkflowDirs, WorkflowId, check_package_path,
    resolve_package_file,
};

const KINDS: [&str; 6] = [
    "editor",
    "explorer",
    "terminal",
    "chat",
    "placeholder",
    "webview",
];

fn manifest(id: &str, web_rule: &str) -> String {
    format!(
        r#"
id = "{id}"
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"
name = "Web"

[layout.center]
panels = ["web"]

[panels]
web = {web_rule}
"#
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
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
fn pages_resolve_inside_the_package_with_exact_case() {
    let temp = tempfile::tempdir().unwrap();
    write(&temp.path().join("ui/Index.html"), "<p>hi</p>");
    let file = resolve_package_file(temp.path(), "ui/Index.html").unwrap();
    assert_eq!(fs::read_to_string(file).unwrap(), "<p>hi</p>");

    // Windows would open these; a case-sensitive system would not.
    assert!(resolve_package_file(temp.path(), "ui/index.html").is_err());
    assert!(resolve_package_file(temp.path(), "UI/Index.html").is_err());
    // Missing, a folder, a file used as a folder.
    assert!(resolve_package_file(temp.path(), "ui/missing.html").is_err());
    assert!(resolve_package_file(temp.path(), "ui").is_err());
    assert!(resolve_package_file(temp.path(), "ui/Index.html/x").is_err());
    // Outside the package.
    assert!(resolve_package_file(&temp.path().join("ui"), "../ui/Index.html").is_err());
}

#[cfg(unix)]
#[test]
fn links_are_not_followed() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(&outside.path().join("secret.txt"), "secret");
    std::os::unix::fs::symlink(outside.path(), temp.path().join("out")).unwrap();
    assert!(resolve_package_file(temp.path(), "out/secret.txt").is_err());
}

#[test]
fn page_is_checked_and_only_for_webviews() {
    let (good, _) = WorkflowDef::parse(&manifest(
        "test.web",
        r#"{ kind = "webview", page = "ui/index.html" }"#,
    ))
    .unwrap();
    assert!(good.validate(false).is_empty());
    assert_eq!(good.panels["web"].page.as_deref(), Some("ui/index.html"));

    let (escape, _) = WorkflowDef::parse(&manifest(
        "test.web",
        r#"{ kind = "webview", page = "../x.html" }"#,
    ))
    .unwrap();
    assert!(escape.validate(false)[0].contains("page"));

    let (wrong_kind, _) = WorkflowDef::parse(&manifest(
        "test.web",
        r#"{ kind = "placeholder", title = "P", page = "index.html" }"#,
    ))
    .unwrap();
    assert!(wrong_kind.validate(false)[0].contains("only for `webview`"));
}

#[test]
fn user_workflows_know_their_package_folder() {
    let temp = tempfile::tempdir().unwrap();
    let dirs = WorkflowDirs::new(temp.path());
    let folder = dirs.user_dir().join("web");
    write(
        &folder.join("workflow.toml"),
        &manifest("test.web", r#"{ kind = "webview", page = "index.html" }"#),
    );
    let registry = Registry::load(Some(&dirs), &KINDS);
    assert_eq!(
        registry.package_dir(&WorkflowId::new("test.web")),
        Some(folder.as_path())
    );
    // Carbon's own shipped packages have one too; the compiled-in fallback not.
    let shipped = Registry::builtin(&KINDS);
    if !shipped.uses_editor_fallback() {
        assert!(shipped.package_dir(&WorkflowId::editor()).is_some());
    }
}
