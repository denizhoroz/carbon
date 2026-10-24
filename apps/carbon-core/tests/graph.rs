//! Graph documents: YAML round trips, the generic checks, and undo/redo.

use std::time::{Duration, Instant};

use carbon_core::graph::{
    Diagnostic, Edge, GraphDoc, GraphEdit, History, Node, NodeTypeSpec, PortRef, PortSpec, Pos,
    Severity, from_yaml, logic_hash, to_yaml, validate,
};
use serde_json::json;

fn port(id: &str, multi: bool, loop_back: bool) -> PortSpec {
    PortSpec {
        id: id.into(),
        label: String::new(),
        multi,
        loop_back,
    }
}

fn spec(id: &str, inputs: Vec<PortSpec>, outputs: Vec<PortSpec>) -> NodeTypeSpec {
    NodeTypeSpec {
        graph: "flow".into(),
        id: id.into(),
        title: id.into(),
        icon: None,
        category: None,
        description: String::new(),
        max_count: None,
        inputs,
        outputs,
        outputs_from: None,
        fields: Vec::new(),
    }
}

/// start, agent (prompt required), loop (back input), router (routes).
fn types() -> Vec<NodeTypeSpec> {
    let mut start = spec("start", vec![], vec![port("out", true, false)]);
    start.max_count = Some(1);
    let mut agent = spec(
        "agent",
        vec![port("in", true, false)],
        vec![port("out", true, false)],
    );
    agent.fields = toml::from_str::<toml::Table>(
        r#"fields = [ { key = "prompt", label = "Prompt", kind = "text", required = true } ]"#,
    )
    .unwrap()["fields"]
        .clone()
        .try_into()
        .unwrap();
    let looping = spec(
        "loop",
        vec![port("in", false, false), port("back", true, true)],
        vec![port("body", false, false), port("done", true, false)],
    );
    let mut router = spec("router", vec![port("in", false, false)], vec![]);
    router.outputs_from = Some("routes".into());
    vec![start, agent, looping, router]
}

fn node(kind: &str, config: serde_json::Value) -> Node {
    let mut node = Node::new(kind);
    if let serde_json::Value::Object(map) = config {
        node.config = map.into_iter().collect();
    }
    node
}

fn edge(id: &str, from: &str, to: &str) -> Edge {
    let split = |text: &str| {
        let (node, port) = text.split_once('.').unwrap();
        PortRef::new(node, port)
    };
    Edge {
        id: id.into(),
        from: split(from),
        to: split(to),
    }
}

fn sample() -> GraphDoc {
    let mut doc = GraphDoc::new("carbon.agents/flow", "Fix tests");
    doc.meta
        .insert("budget".into(), json!({ "max_cost_usd": 2.0 }));
    doc.nodes.insert("start".into(), Node::new("start"));
    doc.nodes.insert(
        "plan".into(),
        node(
            "agent",
            json!({ "prompt": "Plan it\nin two lines", "agent": "claude-code" }),
        ),
    );
    doc.nodes
        .insert("retry".into(), node("loop", json!({ "max": 3 })));
    doc.nodes.insert(
        "check".into(),
        node(
            "router",
            json!({ "routes": [ { "id": "pass", "match": "ok" } ] }),
        ),
    );
    doc.edges = vec![
        edge("e1", "start.out", "plan.in"),
        edge("e2", "plan.out", "retry.in"),
        edge("e3", "retry.body", "check.in"),
        edge("e4", "check.else", "retry.back"),
    ];
    doc.layout.nodes.insert("start".into(), Pos::new(40., 200.));
    doc.layout.nodes.insert("plan".into(), Pos::new(240., 200.));
    doc
}

#[test]
fn yaml_round_trips_byte_for_byte() {
    let doc = sample();
    let text = to_yaml(&doc);
    let back = from_yaml(&text).unwrap();
    assert_eq!(back, doc);
    assert_eq!(
        to_yaml(&back),
        text,
        "an unchanged save writes the same bytes"
    );
    assert!(
        text.starts_with("carbon_graph: 1\nkind: carbon.agents/flow\n"),
        "{text}"
    );
    assert!(text.contains("from: start.out"), "{text}");
    // Unknown top-level keys survive.
    let extra = format!("{text}future_key:\n  a: 1\n");
    let doc = from_yaml(&extra).unwrap();
    assert!(to_yaml(&doc).contains("future_key"));
    assert!(from_yaml("carbon_graph: 2\nkind: x\nname: y\n").is_err());
    assert!(
        from_yaml("carbon_graph: 1\nkind: x\nname: y\nedges:\n  - { id: e, from: a, to: b.in }\n")
            .is_err()
    );
}

#[test]
fn moving_nodes_keeps_the_logic_hash() {
    let doc = sample();
    let mut moved = doc.clone();
    moved.layout.nodes.insert("plan".into(), Pos::new(999., 1.));
    assert_eq!(logic_hash(&doc), logic_hash(&moved));
    let mut changed = doc.clone();
    changed.nodes["plan"].title = Some("Plan".into());
    assert_ne!(logic_hash(&doc), logic_hash(&changed));
}

fn messages(problems: &[Diagnostic]) -> Vec<String> {
    problems.iter().map(|p| p.message.clone()).collect()
}

#[test]
fn a_loop_back_input_is_the_only_cycle_allowed() {
    let doc = sample();
    let problems = validate(&doc, &types());
    assert!(problems.is_empty(), "{:?}", messages(&problems));

    let mut cyclic = doc.clone();
    cyclic.edges.push(edge("e5", "check.pass", "plan.in"));
    let problems = validate(&cyclic, &types());
    let flagged: Vec<_> = problems
        .iter()
        .filter(|p| p.message.contains("circle"))
        .filter_map(|p| p.edge.clone())
        .collect();
    // plan -> retry -> check -> plan; nothing before or after the loop.
    assert_eq!(flagged, ["e2", "e3", "e5"], "{:?}", messages(&problems));
}

#[test]
fn ports_counts_and_required_fields_are_checked() {
    let mut doc = sample();
    doc.nodes.insert("start-2".into(), Node::new("start"));
    doc.nodes.insert("odd".into(), Node::new("mystery"));
    doc.nodes
        .insert("empty".into(), node("agent", json!({ "prompt": "  " })));
    doc.edges.push(edge("e5", "plan.out", "retry.in")); // retry.in takes one
    doc.edges.push(edge("e6", "plan.nope", "empty.in"));
    doc.edges.push(edge("e7", "check.pass", "ghost.in"));
    doc.edges.push(edge("e1", "start.out", "empty.in")); // id used twice
    let problems = validate(&doc, &types());
    let text = messages(&problems).join("\n");
    assert!(text.contains("at most 1 start"), "{text}");
    assert!(text.contains("unknown node type `mystery`"), "{text}");
    assert!(text.contains("Prompt is empty"), "{text}");
    assert!(text.contains("retry.in takes one arrow"), "{text}");
    assert!(text.contains("has no output `nope`"), "{text}");
    assert!(text.contains("missing node"), "{text}");
    assert!(text.contains("used twice"), "{text}");
    let empty = problems
        .iter()
        .find(|p| p.message.contains("Prompt"))
        .unwrap();
    assert_eq!(empty.severity, Severity::Warning);
    // Routes make ports: `pass` and `else` exist, `fail` does not.
    let router = &types()[3];
    let ports: Vec<_> = router
        .outputs_for(&doc.nodes["check"])
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ports, ["pass", "else"]);
}

#[test]
fn every_edit_is_undone_exactly() {
    let base = sample();
    let edits = vec![
        GraphEdit::AddNode {
            id: "new".into(),
            node: Node::new("agent"),
            pos: Pos::new(1., 2.),
            index: 1,
        },
        GraphEdit::remove_node(&base, "plan").unwrap(),
        GraphEdit::MoveNodes(vec![(
            "plan".into(),
            Pos::new(240., 200.),
            Pos::new(0., 0.),
        )]),
        GraphEdit::set_field(&base, "plan", "prompt", Some(json!("other"))).unwrap(),
        GraphEdit::set_field(&base, "plan", "agent", None).unwrap(),
        GraphEdit::SetTitle {
            node: "plan".into(),
            old: None,
            new: Some("Plan".into()),
        },
        GraphEdit::SetDisabled {
            node: "plan".into(),
            disabled: true,
        },
        GraphEdit::SetMeta {
            key: "budget".into(),
            old: base.meta.get("budget").cloned(),
            new: None,
        },
        GraphEdit::Rename {
            old: "plan".into(),
            new: "draft".into(),
        },
        GraphEdit::AddEdge {
            edge: edge("e9", "start.out", "check.in"),
            index: 0,
        },
        GraphEdit::remove_edge(&base, "e3").unwrap(),
    ];
    for edit in edits {
        let mut doc = base.clone();
        let mut history = History::new();
        history.apply(&mut doc, edit.clone(), None);
        assert_ne!(doc, base, "{edit:?} changed nothing");
        assert!(history.undo(&mut doc));
        assert_eq!(doc, base, "undo of {edit:?}");
        assert!(history.redo(&mut doc));
        assert_ne!(doc, base);
        assert!(history.undo(&mut doc));
        assert_eq!(doc, base);
    }
}

#[test]
fn renaming_moves_arrows_and_layout() {
    let mut doc = sample();
    GraphEdit::Rename {
        old: "plan".into(),
        new: "draft".into(),
    }
    .apply(&mut doc);
    assert_eq!(doc.nodes.get_index_of("draft"), Some(1));
    assert_eq!(doc.edges[0].to, PortRef::new("draft", "in"));
    assert_eq!(doc.pos("draft"), Pos::new(240., 200.));
}

#[test]
fn typing_in_one_field_is_one_step() {
    let base = sample();
    let mut doc = base.clone();
    let mut history = History::new();
    let t0 = Instant::now();
    for (ix, text) in ["a", "ab", "abc"].iter().enumerate() {
        let edit = GraphEdit::set_field(&doc, "plan", "prompt", Some(json!(text))).unwrap();
        history.apply_at(
            &mut doc,
            edit,
            Some("plan/prompt".into()),
            t0 + Duration::from_millis(300 * ix as u64),
        );
    }
    // A pause, then more typing: a second step.
    let edit = GraphEdit::set_field(&doc, "plan", "prompt", Some(json!("abcd"))).unwrap();
    history.apply_at(
        &mut doc,
        edit,
        Some("plan/prompt".into()),
        t0 + Duration::from_secs(5),
    );
    assert!(history.undo(&mut doc));
    assert_eq!(doc.nodes["plan"].config["prompt"], json!("abc"));
    assert!(history.undo(&mut doc));
    assert_eq!(doc, base);
    assert!(!history.can_undo());
}

#[test]
fn new_ids_are_free_and_readable() {
    let doc = sample();
    assert_eq!(doc.next_node_id("agent"), "agent");
    assert_eq!(doc.next_node_id("start"), "start-2");
    assert_eq!(doc.next_node_id("Big Thing"), "big-thing");
    assert_eq!(doc.next_edge_id(), "e5");
}
