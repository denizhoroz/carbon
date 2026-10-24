//! Checks every graph must pass, from the node type specs alone: known types,
//! arrows between existing ports in the right direction, port and node-count
//! limits, required fields, and no cycle except through a `loop_back` input.
//! What the nodes mean is checked by the extension (`check-graph`).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use serde_json::Value;

use super::{GraphDoc, NodeTypeSpec, check_graph_id};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// One problem, on a node, an edge, or the whole graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub node: Option<String>,
    pub edge: Option<String>,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    fn node(node: &str, severity: Severity, message: String) -> Self {
        Self {
            node: Some(node.to_string()),
            edge: None,
            severity,
            message,
        }
    }

    fn edge(edge: &str, message: String) -> Self {
        Self {
            node: None,
            edge: Some(edge.to_string()),
            severity: Severity::Error,
            message,
        }
    }
}

/// Every problem in `doc`, given the node types of its kind.
pub fn validate(doc: &GraphDoc, types: &[NodeTypeSpec]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let spec_of = |kind: &str| types.iter().find(|spec| spec.id == kind);

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (id, node) in &doc.nodes {
        if let Err(problem) = check_graph_id(id) {
            problems.push(Diagnostic::node(id, Severity::Error, problem));
        }
        let Some(spec) = spec_of(&node.kind) else {
            problems.push(Diagnostic::node(
                id,
                Severity::Error,
                format!("unknown node type `{}`", node.kind),
            ));
            continue;
        };
        let count = counts.entry(spec.id.as_str()).or_default();
        *count += 1;
        if spec.max_count.is_some_and(|max| *count > max) {
            problems.push(Diagnostic::node(
                id,
                Severity::Error,
                format!(
                    "at most {} {} in a graph",
                    spec.max_count.unwrap_or_default(),
                    spec.title
                ),
            ));
        }
        for field in spec.fields.iter().filter(|field| field.required) {
            let empty = match node.config.get(&field.key) {
                None | Some(Value::Null) => true,
                Some(Value::String(text)) => text.trim().is_empty(),
                Some(Value::Array(items)) => items.is_empty(),
                Some(_) => false,
            };
            if empty && !node.disabled {
                problems.push(Diagnostic::node(
                    id,
                    Severity::Warning,
                    format!("{} is empty", field.label),
                ));
            }
        }
    }

    // Arrows: ends exist, right direction, ports not over-used.
    let mut seen_ids = HashSet::new();
    let mut seen_pairs = HashSet::new();
    let mut uses: BTreeMap<(String, String, bool), Vec<&str>> = BTreeMap::new();
    let mut loop_back_edges = HashSet::new();
    for edge in &doc.edges {
        if !seen_ids.insert(edge.id.as_str()) {
            problems.push(Diagnostic::edge(
                &edge.id,
                format!("edge id `{}` is used twice", edge.id),
            ));
        }
        if !seen_pairs.insert((&edge.from, &edge.to)) {
            problems.push(Diagnostic::edge(&edge.id, "duplicate arrow".into()));
        }
        let from = doc.nodes.get(&edge.from.node);
        let to = doc.nodes.get(&edge.to.node);
        let (Some(from), Some(to)) = (from, to) else {
            problems.push(Diagnostic::edge(
                &edge.id,
                format!("arrow {} -> {} names a missing node", edge.from, edge.to),
            ));
            continue;
        };
        let (Some(from_spec), Some(to_spec)) = (spec_of(&from.kind), spec_of(&to.kind)) else {
            continue;
        };
        let outputs = from_spec.outputs_for(from);
        match outputs.iter().find(|port| port.id == edge.from.port) {
            Some(port) => uses
                .entry((edge.from.node.clone(), edge.from.port.clone(), port.multi))
                .or_default()
                .push(&edge.id),
            None => problems.push(Diagnostic::edge(
                &edge.id,
                format!("{} has no output `{}`", from_spec.title, edge.from.port),
            )),
        }
        match to_spec.input(&edge.to.port) {
            Some(port) => {
                if port.loop_back {
                    loop_back_edges.insert(edge.id.as_str());
                }
                uses.entry((edge.to.node.clone(), edge.to.port.clone(), port.multi))
                    .or_default()
                    .push(&edge.id)
            }
            None => problems.push(Diagnostic::edge(
                &edge.id,
                format!("{} has no input `{}`", to_spec.title, edge.to.port),
            )),
        }
    }
    for ((node, port, multi), edges) in &uses {
        if !multi && edges.len() > 1 {
            for edge in &edges[1..] {
                problems.push(Diagnostic::edge(
                    edge,
                    format!("{node}.{port} takes one arrow"),
                ));
            }
        }
    }

    // Cycles: without the arrows into loop-back inputs, the graph is a DAG.
    let forward: Vec<_> = doc
        .edges
        .iter()
        .filter(|edge| !loop_back_edges.contains(edge.id.as_str()))
        .filter(|edge| {
            doc.nodes.contains_key(&edge.from.node) && doc.nodes.contains_key(&edge.to.node)
        })
        .collect();
    let mut indegree: HashMap<&str, usize> = doc.nodes.keys().map(|id| (id.as_str(), 0)).collect();
    for edge in &forward {
        *indegree.entry(edge.to.node.as_str()).or_default() += 1;
    }
    let mut ready: VecDeque<&str> = indegree
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut done = HashSet::new();
    while let Some(node) = ready.pop_front() {
        done.insert(node);
        for edge in forward.iter().filter(|edge| edge.from.node == node) {
            let n = indegree.get_mut(edge.to.node.as_str()).expect("counted");
            *n -= 1;
            if *n == 0 {
                ready.push_back(edge.to.node.as_str());
            }
        }
    }
    // What is left holds the cycles and what comes after them; peel off,
    // from the end, every node that leads nowhere left, so only the loops
    // themselves are flagged.
    let mut left: HashSet<&str> = doc
        .nodes
        .keys()
        .map(String::as_str)
        .filter(|id| !done.contains(id))
        .collect();
    loop {
        let dead: Vec<&str> = left
            .iter()
            .copied()
            .filter(|id| {
                !forward
                    .iter()
                    .any(|e| e.from.node == *id && left.contains(e.to.node.as_str()))
            })
            .collect();
        if dead.is_empty() {
            break;
        }
        for id in dead {
            left.remove(id);
        }
    }
    for edge in &forward {
        if left.contains(edge.from.node.as_str()) && left.contains(edge.to.node.as_str()) {
            problems.push(Diagnostic::edge(
                &edge.id,
                "arrows go round in a circle; use a loop's way-back input".into(),
            ));
        }
    }
    problems
}
