//! Workflow system Phase 2: user folder, overlays, state file, crash guard.

use std::fs;
use std::path::Path;

use carbon_core::workflow::{
    Origin, Overlay, Registry, SavedWorkflow, StateFile, WorkflowDef, WorkflowDirs, WorkflowId,
    finish_entering, start_entering, take_unfinished,
};
use serde_json::json;

const KINDS: [&str; 5] = ["editor", "explorer", "terminal", "chat", "placeholder"];

fn user_manifest(id: &str, name: &str, order: i32) -> String {
    format!(
        r#"
id = "{id}"
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"
name = "{name}"
order = {order}

[layout.center]
panels = ["editor", "chat"]

[layout.bottom]
panels = ["terminal"]
open = false

[panels]
editor = {{ multi = true }}
chat = {{}}
terminal = {{}}

[[commands]]
id = "new-file"
label = "New Review File"
run = "carbon::NewFile"

[[menus]]
menu = "File"
command = "new-file"
"#
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn dirs() -> (tempfile::TempDir, WorkflowDirs) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = WorkflowDirs::new(temp.path());
    (temp, dirs)
}

fn ids(registry: &Registry) -> Vec<&str> {
    registry.available().map(|def| def.id.as_str()).collect()
}

#[test]
fn user_workflows_load_after_builtins_and_bad_ones_are_skipped() {
    let (_temp, dirs) = dirs();
    write(
        &dirs.user_dir().join("review/workflow.toml"),
        &user_manifest("me.review", "Review", 150),
    );
    write(&dirs.user_dir().join("broken/workflow.toml"), "id = ");
    write(
        &dirs.user_dir().join("thief/workflow.toml"),
        &user_manifest("carbon.editor", "Thief", 1),
    );
    write(
        &dirs.user_dir().join("twin/workflow.toml"),
        &user_manifest("me.review", "Twin", 2),
    );
    fs::create_dir_all(dirs.user_dir().join("empty")).unwrap();

    let registry = Registry::load(Some(&dirs), &KINDS);
    assert_eq!(
        ids(&registry),
        [
            "carbon.editor",
            "me.review",
            "carbon.paradigm-editor",
            "carbon.agent-automation"
        ]
    );
    let review = WorkflowId::new("me.review");
    assert!(matches!(registry.origin(&review), Some(Origin::User(_))));
    assert_eq!(
        registry.get(&review).unwrap().commands[0].run,
        "carbon::NewFile"
    );
    let rejected: Vec<String> = registry
        .rejected()
        .iter()
        .map(|r| r.source.replace('\\', "/"))
        .collect();
    assert_eq!(rejected.len(), 3, "{rejected:?}");
    assert!(rejected.iter().any(|s| s.ends_with("broken/workflow.toml")));
    assert!(rejected.iter().any(|s| s.ends_with("thief/workflow.toml")));
    assert!(rejected.iter().any(|s| s.ends_with("twin/workflow.toml")));
}

#[test]
fn no_dirs_means_builtins_only() {
    let registry = Registry::load(None, &KINDS);
    assert_eq!(registry.workflows().len(), 3);
}

#[test]
fn commands_and_menus_are_validated() {
    let text = user_manifest("me.review", "Review", 1)
        .replace("command = \"new-file\"", "command = \"nope\"")
        .replace("menu = \"File\"", "menu = \"Tools\"");
    let (def, _) = WorkflowDef::parse(&text).unwrap();
    let problems = def.validate(false);
    assert!(problems.iter().any(|p| p.contains("no command `nope`")));
    assert!(problems.iter().any(|p| p.contains("menu `Tools`")));
    let twice = user_manifest("me.review", "Review", 1).replace(
        "[[menus]]",
        "[[commands]]\nid = \"new-file\"\nlabel = \"Again\"\nrun = \"carbon::NewFile\"\n\n[[menus]]",
    );
    let (def, _) = WorkflowDef::parse(&twice).unwrap();
    assert!(def.validate(false).iter().any(|p| p.contains("used twice")));
}

#[test]
fn overlay_patches_a_builtin_and_deleting_it_resets() {
    let (_temp, dirs) = dirs();
    write(
        &dirs.overlay_file("carbon.editor"),
        r#"
name = "Code"
order = 500
[layout.right]
remove = ["chat"]
open = false
[layout.bottom]
add = ["chat2"]
[panels]
chat2 = { kind = "chat" }
"#,
    );
    write(
        &dirs.overlay_file("carbon.paradigm-editor"),
        "hidden = true",
    );
    let registry = Registry::load(Some(&dirs), &KINDS);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    let editor = registry.get(&WorkflowId::editor()).unwrap();
    assert_eq!(editor.name, "Code");
    let right = editor.layout.right.as_ref().unwrap();
    assert!(right.panels.is_empty() && !right.open);
    assert_eq!(
        editor.layout.bottom.as_ref().unwrap().panels,
        ["terminal", "chat2"]
    );
    assert!(registry.overlay(&WorkflowId::editor()).is_some());
    assert_eq!(
        ids(&registry),
        ["carbon.agent-automation", "carbon.editor"],
        "order changed, paradigm hidden"
    );

    dirs.remove_overlay("carbon.editor").unwrap();
    dirs.remove_overlay("carbon.editor").unwrap();
    let registry = Registry::load(Some(&dirs), &KINDS);
    assert_eq!(registry.get(&WorkflowId::editor()).unwrap().name, "Editor");
}

#[test]
fn bad_overlays_are_rejected_and_the_base_kept() {
    let (_temp, dirs) = dirs();
    write(&dirs.overlay_file("carbon.editor"), "id = \"me.mine\"");
    write(
        &dirs.overlay_file("carbon.agent-automation"),
        "[layout.center]\npanels = []",
    );
    write(&dirs.overlay_file("me.nothing"), "name = \"x\"");
    write(
        &dirs.user_dir().join("review/workflow.toml"),
        &user_manifest("me.review", "Review", 1),
    );
    write(&dirs.overlay_file("me.review"), "name = \"x\"");
    write(
        &dirs.overlay_file("carbon.paradigm-editor"),
        "[panels]\ncanvas = { kind = \"chat\" }",
    );
    let registry = Registry::load(Some(&dirs), &KINDS);
    let problems: Vec<String> = registry
        .rejected()
        .iter()
        .flat_map(|r| r.problems.clone())
        .collect();
    assert!(problems.iter().any(|p| p.contains("unknown field `id`")));
    assert!(
        problems
            .iter()
            .any(|p| p.contains("layout.center needs at least one panel"))
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("no workflow `me.nothing`"))
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("edit its workflow.toml"))
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("panel `canvas` already exists"))
    );
    assert_eq!(registry.get(&WorkflowId::editor()).unwrap().name, "Editor");
    assert_eq!(
        registry
            .get(&WorkflowId::new("carbon.agent-automation"))
            .unwrap()
            .layout
            .center
            .as_ref()
            .unwrap()
            .panels,
        ["runs"]
    );
}

#[test]
fn overlay_remove_of_a_missing_panel_is_a_notice() {
    let base = Registry::builtin(&KINDS);
    let editor = base.get(&WorkflowId::editor()).unwrap();
    let overlay = Overlay::parse("[layout.left]\nremove = [\"ghost\"]").unwrap();
    let (_, notices) = overlay.apply(editor).unwrap();
    assert!(notices[0].contains("remove `ghost` did nothing"));
    let overlay = Overlay::parse("[layout.top]\nadd = [\"chat\"]").unwrap();
    assert!(overlay.apply(editor).unwrap_err()[0].contains("not a region"));
}

#[test]
fn blocked_and_hidden_workflows_are_not_offered() {
    let mut registry = Registry::builtin(&KINDS);
    registry.block([WorkflowId::new("carbon.agent-automation")]);
    assert_eq!(ids(&registry), ["carbon.editor", "carbon.paradigm-editor"]);
    assert!(
        registry
            .get(&WorkflowId::new("carbon.agent-automation"))
            .is_some()
    );
    registry.block([WorkflowId::editor()]);
    assert!(
        registry.is_available(&WorkflowId::editor()),
        "the Editor is never blocked"
    );
    assert_eq!(
        registry.default_workflow().unwrap().id.as_str(),
        "carbon.editor"
    );
}

#[test]
fn fingerprint_follows_the_layout_only() {
    let registry = Registry::builtin(&KINDS);
    let editor = registry.get(&WorkflowId::editor()).unwrap();
    let renamed = Overlay::parse("name = \"Code\"\nicon = \"bot\"")
        .unwrap()
        .apply(editor)
        .unwrap()
        .0;
    assert_eq!(renamed.layout_fingerprint(), editor.layout_fingerprint());
    let moved = Overlay::parse("[layout.right]\nopen = false")
        .unwrap()
        .apply(editor)
        .unwrap()
        .0;
    assert_ne!(moved.layout_fingerprint(), editor.layout_fingerprint());
    assert_eq!(
        editor.base_panels(),
        [
            "center:editor",
            "left:explorer",
            "right:chat",
            "bottom:terminal"
        ]
    );

    let saved = SavedWorkflow::new(editor, json!({"dock": 1}));
    assert!(saved.matches(&renamed));
    assert!(!saved.matches(&moved));
}

#[test]
fn state_file_round_trips_and_missing_or_corrupt_is_reported() {
    let (_temp, dirs) = dirs();
    let path = dirs.state_file();
    assert_eq!(StateFile::load(&path), Ok(None));

    let registry = Registry::builtin(&KINDS);
    let editor = registry.get(&WorkflowId::editor()).unwrap();
    let mut state = StateFile {
        active: Some(WorkflowId::editor()),
        ..StateFile::default()
    };
    state.workflows.insert(
        WorkflowId::editor(),
        SavedWorkflow::new(editor, json!({"a": 1})),
    );
    state.disabled.insert(WorkflowId::new("me.off"));
    state
        .quarantined
        .insert(WorkflowId::new("me.bad"), "crashed".into());
    state.save(&path).unwrap();
    assert_eq!(StateFile::load(&path).unwrap(), Some(state));

    fs::write(&path, "{ not json").unwrap();
    assert!(StateFile::load(&path).is_err());
    StateFile::set_aside(&path);
    assert!(!path.exists() && path.with_extension("json.bad").exists());
}

#[test]
fn entering_mark_survives_a_crash_only() {
    let (_temp, dirs) = dirs();
    let review = WorkflowId::new("me.review");
    assert_eq!(take_unfinished(&dirs), None);

    start_entering(&dirs, &review).unwrap();
    finish_entering(&dirs);
    assert_eq!(
        take_unfinished(&dirs),
        None,
        "a finished opening leaves no mark"
    );

    start_entering(&dirs, &review).unwrap();
    // The app dies here.
    assert_eq!(take_unfinished(&dirs), Some(review));
    assert_eq!(take_unfinished(&dirs), None, "taking clears the mark");
}

#[test]
fn byte_order_mark_is_ignored() {
    let text = format!("\u{feff}{}", user_manifest("me.review", "Review", 1));
    assert!(WorkflowDef::parse(&text).is_ok());
    assert!(Overlay::parse("\u{feff}name = \"x\"").is_ok());
}

/// The repo's `apps/carbon-workflow`, the shipped folder in development.
fn repo_shipped() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../carbon-workflow")
}

#[test]
fn carbon_workflows_load_from_shipped_package_folders() {
    let registry = Registry::shipped(Some(&repo_shipped()), &KINDS);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    assert!(registry.warnings().is_empty(), "{:?}", registry.warnings());
    assert_eq!(registry.workflows().len(), 3);
    for def in registry.workflows() {
        assert_eq!(registry.origin(&def.id), Some(&Origin::Builtin));
    }
    for folder in ["editor", "paradigm", "agent"] {
        assert!(repo_shipped().join(folder).join("workflow.toml").is_file());
    }
}

#[test]
fn editor_falls_back_to_the_compiled_copy_without_the_shipped_folder() {
    let registry = Registry::shipped(None, &KINDS);
    let ids: Vec<&str> = registry.workflows().iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["carbon.editor"]);
    assert!(
        registry.warnings()[0]
            .1
            .contains("using the built-in Editor")
    );
    assert!(registry.uses_editor_fallback());
    assert!(!Registry::shipped(Some(&repo_shipped()), &KINDS).uses_editor_fallback());

    // A shipped folder without the Editor still gets it.
    let (_temp, dirs) = dirs();
    let shipped = dirs.root().join("shipped");
    let paradigm = fs::read_to_string(repo_shipped().join("paradigm/workflow.toml")).unwrap();
    write(&shipped.join("paradigm/workflow.toml"), &paradigm);
    let registry = Registry::shipped(Some(&shipped), &KINDS);
    let ids: Vec<&str> = registry.workflows().iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["carbon.editor", "carbon.paradigm-editor"]);
}

#[test]
fn installed_packages_use_their_newest_version() {
    let (_temp, dirs) = dirs();
    let package = dirs.installed_dir().join("me.review");
    write(
        &package.join("1.2.0/workflow.toml"),
        &user_manifest("me.review", "Review 1.2", 150),
    );
    write(
        &package.join("1.10.0/workflow.toml"),
        &user_manifest("me.review", "Review 1.10", 150),
    );
    write(&package.join("latest/workflow.toml"), "not read");
    write(
        &dirs.installed_dir().join("thief/1.0.0/workflow.toml"),
        &user_manifest("carbon.thief", "Thief", 1),
    );
    write(
        &dirs.user_dir().join("copy/workflow.toml"),
        &user_manifest("me.review", "Copy", 1),
    );
    write(&dirs.overlay_file("me.review"), "name = \"Patched\"");

    let registry = Registry::load(Some(&dirs), &KINDS);
    let review = registry.get(&WorkflowId::new("me.review")).unwrap();
    assert_eq!(review.name, "Patched", "installed packages take overlays");
    assert!(matches!(
        registry.origin(&review.id),
        Some(Origin::Installed(path)) if path.to_string_lossy().contains("1.10.0")
    ));
    let rejected: Vec<String> = registry
        .rejected()
        .iter()
        .map(|r| r.source.replace('\\', "/"))
        .collect();
    assert!(
        rejected
            .iter()
            .any(|s| s.ends_with("thief/1.0.0/workflow.toml")),
        "{rejected:?}"
    );
    assert!(
        rejected.iter().any(|s| s.ends_with("copy/workflow.toml")),
        "{rejected:?}"
    );
}

#[test]
fn only_user_and_installed_workflows_can_be_removed_and_the_editor_always_stays() {
    let (_temp, dirs) = dirs();
    write(
        &dirs.user_dir().join("review/workflow.toml"),
        &user_manifest("me.review", "Review", 150),
    );
    let registry = Registry::load(Some(&dirs), &KINDS);
    let editor = WorkflowId::editor();
    let paradigm = WorkflowId::new("carbon.paradigm-editor");
    let review = WorkflowId::new("me.review");
    assert!(!registry.removable(&editor) && !registry.can_turn_off(&editor));
    assert!(!registry.removable(&paradigm) && registry.can_turn_off(&paradigm));
    assert!(registry.removable(&review) && registry.can_turn_off(&review));
}

#[test]
fn duplicate_writes_a_valid_user_copy_that_loads() {
    let (_temp, dirs) = dirs();
    let registry = Registry::load(Some(&dirs), &KINDS);
    let editor = registry.get(&WorkflowId::editor()).unwrap();
    let id = registry.free_user_id("Editor (copy)");
    assert_eq!(id.as_str(), "user.editor-copy");
    let copy = editor.duplicate(id.clone(), "Editor (copy)");
    assert_eq!(copy.layout_fingerprint(), editor.layout_fingerprint());
    let path = dirs.create_user_workflow(&copy).unwrap();
    assert!(path.ends_with("user/editor-copy/workflow.toml"));
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("derived_from"), "{text}");
    assert!(!text.contains("legacy_ids"), "{text}");

    let registry = Registry::load(Some(&dirs), &KINDS);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    assert!(registry.warnings().is_empty(), "{:?}", registry.warnings());
    let loaded = registry.get(&id).unwrap();
    assert_eq!(loaded.name, "Editor (copy)");
    assert_eq!(loaded.order, 101);
    assert_eq!(loaded.layout, editor.layout);
    assert_eq!(loaded.panels, editor.panels);
    assert_eq!(
        registry.free_user_id("Editor (copy)").as_str(),
        "user.editor-copy-2"
    );

    // A second copy with the same folder name gets its own folder.
    let again = editor.duplicate(WorkflowId::new("other.editor-copy"), "Again");
    let path = dirs.create_user_workflow(&again).unwrap();
    assert!(path.ends_with("user/editor-copy-2/workflow.toml"));
}

#[test]
fn free_user_id_makes_a_slug() {
    let registry = Registry::builtin(&KINDS);
    assert_eq!(
        registry.free_user_id("  My Flow!! 2 ").as_str(),
        "user.my-flow-2"
    );
    assert_eq!(registry.free_user_id("???").as_str(), "user.workflow");
}

#[test]
fn removing_deletes_only_user_and_installed_folders() {
    let (_temp, dirs) = dirs();
    let user = dirs.user_dir().join("review/workflow.toml");
    write(&user, &user_manifest("me.review", "Review", 1));
    let installed = dirs.installed_dir().join("me.pack/1.0.0/workflow.toml");
    write(&installed, &user_manifest("me.pack", "Pack", 2));
    write(
        &dirs.installed_dir().join("me.pack/0.9.0/workflow.toml"),
        "old",
    );

    dirs.remove_workflow(&Origin::User(user.clone())).unwrap();
    assert!(!user.parent().unwrap().exists());
    assert!(dirs.user_dir().exists(), "the user folder itself stays");
    dirs.remove_workflow(&Origin::Installed(installed.clone()))
        .unwrap();
    assert!(
        !dirs.installed_dir().join("me.pack").exists(),
        "all versions go"
    );

    assert!(dirs.remove_workflow(&Origin::Builtin).is_err());
    let outside = tempfile::tempdir().unwrap();
    let stray = outside.path().join("x/workflow.toml");
    write(&stray, "x");
    assert!(dirs.remove_workflow(&Origin::User(stray.clone())).is_err());
    assert!(stray.exists(), "nothing outside the user folder is deleted");
    let root_manifest = dirs.user_dir().join("workflow.toml");
    write(&root_manifest, "x");
    assert!(
        dirs.remove_workflow(&Origin::User(root_manifest)).is_err(),
        "never the user folder itself"
    );
}

#[test]
fn named_layouts_round_trip_in_the_state_file() {
    let (_temp, dirs) = dirs();
    let mut state = StateFile::default();
    state.layouts.insert(
        WorkflowId::editor(),
        json!({"saved": [], "auto_hide": ["left"]}),
    );
    state.save(&dirs.state_file()).unwrap();
    let back = StateFile::load(&dirs.state_file()).unwrap().unwrap();
    assert_eq!(back.layouts[&WorkflowId::editor()]["auto_hide"][0], "left");
    // Files from before named layouts moved here still load.
    fs::write(
        dirs.state_file(),
        r#"{"schema":1,"active":"carbon.editor"}"#,
    )
    .unwrap();
    assert!(
        StateFile::load(&dirs.state_file())
            .unwrap()
            .unwrap()
            .layouts
            .is_empty()
    );
}
