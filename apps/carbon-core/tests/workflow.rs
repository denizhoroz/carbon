use std::collections::HashMap;

use carbon_core::workflow::{LayoutFile, Registry, WorkflowDef, WorkflowId};
use serde::Deserialize;

const KINDS: [&str; 5] = ["editor", "explorer", "terminal", "chat", "placeholder"];

/// A minimal valid user manifest; tests change one thing at a time.
fn manifest(extra: &str) -> String {
    format!(
        r#"
id = "me.review"
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"
name = "Review"
{extra}
[layout.center]
panels = ["editor"]

[panels]
editor = {{}}
"#
    )
}

fn problems(text: &str) -> Vec<String> {
    let (def, _) = WorkflowDef::parse(text).expect("parses");
    def.validate(false)
}

fn has(problems: &[String], part: &str) -> bool {
    problems.iter().any(|problem| problem.contains(part))
}

#[test]
fn builtins_load_in_order() {
    let registry = Registry::builtin(&KINDS);
    assert!(registry.rejected().is_empty(), "{:?}", registry.rejected());
    assert!(registry.warnings().is_empty(), "{:?}", registry.warnings());
    let ids: Vec<&str> = registry
        .workflows()
        .iter()
        .map(|def| def.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "carbon.editor",
            "carbon.paradigm-editor",
            "carbon.agent-automation"
        ]
    );
    assert_eq!(
        registry.default_workflow().unwrap().id,
        WorkflowId::editor()
    );
}

#[test]
fn editor_manifest_matches_the_code_layout() {
    let registry = Registry::builtin(&KINDS);
    let editor = registry.resolve("code").expect("legacy id resolves");
    assert_eq!(editor.id, WorkflowId::editor());
    assert_eq!(editor.name, "Editor");
    let kinds = |region: &Option<carbon_core::workflow::Region>| -> Vec<String> {
        let region = region.as_ref().expect("region set");
        assert!(region.open);
        assert_eq!(region.size, None, "host default size");
        region
            .panels
            .iter()
            .map(|name| editor.panel_kind(name).to_string())
            .collect()
    };
    assert_eq!(kinds(&editor.layout.center), ["editor"]);
    assert_eq!(kinds(&editor.layout.left), ["explorer"]);
    assert_eq!(kinds(&editor.layout.right), ["chat"]);
    assert_eq!(kinds(&editor.layout.bottom), ["terminal"]);
    assert!(!editor.panels["explorer"].closable);
}

#[test]
fn placeholders_have_a_titled_center_panel() {
    let registry = Registry::builtin(&KINDS);
    for id in ["carbon.paradigm-editor", "carbon.agent-automation"] {
        let def = registry.get(&WorkflowId::new(id)).unwrap();
        let center = def.layout.center.as_ref().unwrap();
        assert_eq!(center.panels.len(), 1);
        assert_eq!(def.panel_kind(&center.panels[0]), "placeholder");
        assert_eq!(
            def.panels[&center.panels[0]].title.as_deref(),
            Some(def.name.as_str())
        );
        assert!(def.layout.bottom.is_none());
    }
    // Every workflow gets the Explorer first in its Explorer section.
    let left_kinds = |id: &str| -> Vec<String> {
        let def = registry.get(&WorkflowId::new(id)).unwrap();
        let left = def.layout.left.as_ref().unwrap();
        assert!(left.open);
        left.panels
            .iter()
            .map(|name| def.panel_kind(name).to_string())
            .collect()
    };
    assert_eq!(left_kinds("carbon.agent-automation"), ["explorer"]);
    assert_eq!(
        left_kinds("carbon.paradigm-editor"),
        ["explorer", "placeholder"]
    );
}

#[test]
fn the_explorer_lives_in_the_explorer_section() {
    // Added first to `layout.left`, which is created open when missing.
    let registry = Registry::from_sources(&[("r.toml", &manifest(""))], &KINDS, false);
    let def = &registry.workflows()[0];
    let left = def.layout.left.as_ref().unwrap();
    assert_eq!(left.panels, ["explorer"]);
    assert!(left.open);
    assert!(!def.panels["explorer"].closable);

    // A left region of its own keeps its panels and settings, Explorer first.
    let text = manifest("").replace(
        "[panels]",
        "[layout.left]
panels = [\"notes\"]
open = false

[panels]
notes = { kind = \"placeholder\", title = \"Notes\" }",
    );
    let registry = Registry::from_sources(&[("r.toml", &text)], &KINDS, false);
    let left = registry.workflows()[0].layout.left.clone().unwrap();
    assert_eq!(left.panels, ["explorer", "notes"]);
    assert!(!left.open);

    // Listed by the manifest: kept where it is, not added twice.
    let text = manifest("").replace(
        "[panels]",
        "[layout.left]
panels = [\"notes\", \"files\"]

[panels]
notes = { kind = \"placeholder\", title = \"Notes\" }
files = { kind = \"explorer\" }",
    );
    let registry = Registry::from_sources(&[("r.toml", &text)], &KINDS, false);
    let def = &registry.workflows()[0];
    assert_eq!(def.layout.left.as_ref().unwrap().panels, ["notes", "files"]);
    assert!(!def.panels.contains_key("explorer"));

    // Anywhere else, twice, or the name taken: rejected.
    let text = manifest("").replace(
        "panels = [\"editor\"]",
        "panels = [\"editor\", \"explorer\"]",
    );
    let text = text.replace(
        "editor = {}",
        "editor = {}
explorer = {}",
    );
    assert!(has(
        &problems(&text),
        "layout.center: the Explorer stays in layout.left"
    ));
    let text = manifest("").replace(
        "[panels]",
        "[layout.left]
panels = [\"explorer\", \"files\"]

[panels]
explorer = {}
files = { kind = \"explorer\" }",
    );
    assert!(has(&problems(&text), "listed more than once"));
    let text = manifest("").replace(
        "editor = {}",
        "editor = {}
explorer = { kind = \"placeholder\", title = \"Files\" }",
    );
    assert!(has(&problems(&text), "the name is kept for the Explorer"));
}

#[test]
fn valid_user_manifest_has_no_problems() {
    assert_eq!(problems(&manifest("")), Vec::<String>::new());
}

#[test]
fn bad_ids_are_rejected() {
    for id in [
        "review",
        "Me.review",
        "me..review",
        "me.re view",
        "-me.review",
    ] {
        let text = manifest("").replace("me.review", id);
        assert!(has(&problems(&text), "publisher.name"), "{id}");
    }
    let text = manifest("").replace("me.review", "carbon.review");
    assert!(has(&problems(&text), "reserved"));
}

#[test]
fn unknown_panel_kind_warns_and_missing_reference_is_rejected() {
    let text = manifest("").replace("editor = {}", "editor = { kind = \"spreadsheet\" }");
    assert!(problems(&text).is_empty(), "an unknown kind only warns");
    let registry = Registry::from_sources(&[("s.toml", &text)], &KINDS, false);
    assert_eq!(registry.workflows().len(), 1);
    assert!(
        registry.warnings()[0]
            .1
            .contains("kind `spreadsheet` is not available")
    );
    let text = manifest("").replace("panels = [\"editor\"]", "panels = [\"editor\", \"nope\"]");
    assert!(has(&problems(&text), "panel `nope` is not in [panels]"));
}

#[test]
fn placeholder_needs_title_and_center_needs_a_panel() {
    let text = manifest("").replace("editor = {}", "editor = { kind = \"placeholder\" }");
    assert!(has(&problems(&text), "needs a title"));
    let text = manifest("").replace("panels = [\"editor\"]", "panels = []");
    assert!(has(
        &problems(&text),
        "layout.center needs at least one panel"
    ));
}

#[test]
fn api_range_and_versions_are_checked() {
    let newer = manifest("").replace(">=0.1, <0.2", ">=0.2");
    assert!(has(&problems(&newer), "needs Carbon API >=0.2"));
    let garbage = manifest("").replace(">=0.1, <0.2", "soon");
    assert!(has(&problems(&garbage), "not a version range"));
    let version = manifest("").replace("version = \"1.0.0\"", "version = \"one\"");
    assert!(has(&problems(&version), "not semver"));
    let schema = manifest("").replace("schema_version = 1", "schema_version = 2");
    assert!(has(&problems(&schema), "schema_version 2"));
}

#[test]
fn unknown_keys_warn_at_top_level_and_fail_in_strict_tables() {
    let (_, warnings) = WorkflowDef::parse(&manifest("colour = \"red\"\ncommands = []")).unwrap();
    assert_eq!(
        warnings,
        ["unknown key `colour` ignored"],
        "reserved keys are silent"
    );
    let strict = manifest("").replace(
        "panels = [\"editor\"]",
        "panels = [\"editor\"]\nfixed = true",
    );
    assert!(WorkflowDef::parse(&strict).is_err());
}

#[test]
fn one_bad_manifest_does_not_stop_the_others() {
    let good = manifest("");
    let other = manifest("").replace("me.review", "me.other");
    let duplicate = manifest("");
    let registry = Registry::from_sources(
        &[
            ("good.toml", &good),
            ("broken.toml", "id = "),
            ("dup.toml", &duplicate),
            ("other.toml", &other),
        ],
        &KINDS,
        false,
    );
    let ids: Vec<&str> = registry.workflows().iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["me.other", "me.review"]);
    let rejected: Vec<&str> = registry
        .rejected()
        .iter()
        .map(|r| r.source.as_str())
        .collect();
    assert_eq!(rejected, ["broken.toml", "dup.toml"]);
    assert!(has(
        &registry.rejected()[1].problems,
        "already used by good.toml"
    ));
}

/// `layout.json` as written before workflows (one `code` section).
const OLD_FILE: &str = r#"{ "active": "code", "sections": { "code": "editor state" } }"#;

#[test]
fn old_layout_file_loads_as_the_editor() {
    let registry = Registry::builtin(&KINDS);
    let file: LayoutFile<String> = serde_json::from_str(OLD_FILE).unwrap();
    let (active, states) = file.into_parts(&registry);
    assert_eq!(active, Some(WorkflowId::editor()));
    assert_eq!(states[&WorkflowId::editor()], "editor state");
}

/// What a build from before workflows deserializes `layout.json` into.
#[derive(Deserialize)]
#[serde(default)]
#[derive(Default)]
struct OldLayoutFile {
    active: Option<OldSection>,
    sections: HashMap<OldSection, String>,
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "lowercase")]
enum OldSection {
    Code,
}

#[test]
fn new_layout_file_round_trips_and_old_builds_still_read_it() {
    let registry = Registry::builtin(&KINDS);
    let paradigm = WorkflowId::new("carbon.paradigm-editor");
    let gone = WorkflowId::new("me.uninstalled");
    let states = [
        (WorkflowId::editor(), "editor".to_string()),
        (paradigm.clone(), "paradigm".to_string()),
        (gone.clone(), "kept".to_string()),
    ];
    let file = LayoutFile::new(&registry, &paradigm, states);
    let json = serde_json::to_string(&file).unwrap();

    let old: OldLayoutFile = serde_json::from_str(&json).expect("old build reads it");
    assert_eq!(old.active, Some(OldSection::Code));
    assert_eq!(old.sections[&OldSection::Code], "editor");

    let back: LayoutFile<String> = serde_json::from_str(&json).unwrap();
    let (active, states) = back.into_parts(&registry);
    assert_eq!(active, Some(paradigm.clone()));
    assert_eq!(states[&WorkflowId::editor()], "editor");
    assert_eq!(states[&paradigm], "paradigm");
    assert_eq!(states[&gone], "kept", "unknown workflows keep their layout");
}

#[test]
fn editor_active_writes_the_old_format_only() {
    let registry = Registry::builtin(&KINDS);
    let file = LayoutFile::new(
        &registry,
        &WorkflowId::editor(),
        [(WorkflowId::editor(), "editor".to_string())],
    );
    let json = serde_json::to_string(&file).unwrap();
    assert_eq!(json, r#"{"active":"code","sections":{"code":"editor"}}"#);
}

#[test]
fn unknown_active_workflow_falls_back() {
    let registry = Registry::builtin(&KINDS);
    let json = r#"{ "active": "code", "sections": {}, "active_workflow": "me.gone" }"#;
    let file: LayoutFile<String> = serde_json::from_str(json).unwrap();
    assert_eq!(file.into_parts(&registry).0, Some(WorkflowId::editor()));
    let json = r#"{ "active": "nope", "sections": {} }"#;
    let file: LayoutFile<String> = serde_json::from_str(json).unwrap();
    assert_eq!(file.into_parts(&registry).0, None);
}
