//! Declarative views, tier 1: parsing, validation, limits, round trip.

use carbon_core::workflow::{Align, ButtonStyle, MAX_VIEW_DEPTH, Widget, WorkflowDef};

fn manifest(rule: &str, views: &str) -> String {
    format!(
        r#"
id = "test.views"
schema_version = 1
version = "1.0.0"
carbon_api = ">=0.1, <0.2"
name = "Views"

[layout.center]
panels = ["home"]

[panels]
home = {rule}

[[commands]]
id = "new-file"
label = "New File"
run = "carbon::NewFile"

{views}
"#
    )
}

const HOME: &str = r#"
[[views.home]]
type = "column"
align = "center"
children = [
  { type = "heading", text = "Welcome" },
  { type = "text", text = "Start here.", muted = true },
  { type = "row", children = [
    { type = "button", label = "New File", command = "new-file", style = "primary", icon = "file-plus" },
  ] },
]

[[views.home]]
type = "divider"

[[views.home]]
type = "list"
items = [
  { label = "New file", description = "An empty editor tab", command = "new-file" },
  { label = "Read me" },
]

[[views.home]]
type = "markdown"
text = "**Bold** and [a link](https://example.com)."
"#;

fn parse(text: &str) -> WorkflowDef {
    WorkflowDef::parse(text).unwrap().0
}

#[test]
fn a_view_parses_and_validates() {
    let def = parse(&manifest(
        r#"{ kind = "view", title = "Home", view = "home" }"#,
        HOME,
    ));
    assert_eq!(def.validate(false), Vec::<String>::new());
    let home = &def.views["home"];
    assert_eq!(home.len(), 4);
    let Widget::Column { children, align } = &home[0] else {
        panic!("column expected");
    };
    assert_eq!(*align, Align::Center);
    assert!(matches!(
        &children[2],
        Widget::Row { children } if matches!(&children[0], Widget::Button { style: ButtonStyle::Primary, .. })
    ));
    assert_eq!(home[1], Widget::Divider);
}

#[test]
fn unknown_widgets_and_fields_are_rejected() {
    let bad_type = "[[views.home]]\ntype = \"slider\"\n";
    assert!(
        WorkflowDef::parse(&manifest(r#"{ kind = "view", view = "home" }"#, bad_type)).is_err()
    );
    let bad_field = "[[views.home]]\ntype = \"text\"\ntext = \"x\"\ncolor = \"red\"\n";
    assert!(
        WorkflowDef::parse(&manifest(r#"{ kind = "view", view = "home" }"#, bad_field)).is_err()
    );
}

#[test]
fn commands_must_exist() {
    let view = "[[views.home]]\ntype = \"button\"\nlabel = \"Go\"\ncommand = \"missing\"\n";
    let def = parse(&manifest(r#"{ kind = "view", view = "home" }"#, view));
    let problems = def.validate(false);
    assert!(
        problems.iter().any(|p| p.contains("no command `missing`")),
        "{problems:?}"
    );
}

#[test]
fn view_rules_are_checked() {
    let missing = parse(&manifest(r#"{ kind = "view", view = "other" }"#, HOME));
    assert!(missing.validate(false)[0].contains("no view `other`"));
    let no_view = parse(&manifest(r#"{ kind = "view" }"#, HOME));
    assert!(no_view.validate(false)[0].contains("needs `view`"));
    let wrong_kind = parse(&manifest(
        r#"{ kind = "placeholder", title = "P", view = "home" }"#,
        HOME,
    ));
    assert!(wrong_kind.validate(false)[0].contains("only for `view`"));
}

#[test]
fn nesting_is_limited() {
    let mut inner = r#"{ type = "text", text = "deep" }"#.to_string();
    for _ in 0..MAX_VIEW_DEPTH {
        inner = format!(r#"{{ type = "column", children = [{inner}] }}"#);
    }
    let view = format!("[[views.home]]\ntype = \"column\"\nchildren = [{inner}]\n");
    let def = parse(&manifest(r#"{ kind = "view", view = "home" }"#, &view));
    assert!(
        def.validate(false)
            .iter()
            .any(|p| p.contains("nests deeper"))
    );
}

#[test]
fn views_survive_duplicate() {
    let def = parse(&manifest(r#"{ kind = "view", view = "home" }"#, HOME));
    let text = def.to_toml().unwrap();
    assert_eq!(parse(&text).views, def.views);
}
