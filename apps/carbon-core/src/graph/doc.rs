//! The graph document: nodes keyed by stable id, edges between ports, and the
//! layout kept apart from the logic so moving a node is not a logic change.
//!
//! ```yaml
//! carbon_graph: 1
//! kind: carbon.agents/flow
//! name: Fix failing tests
//! nodes:
//!   start: { type: start }
//!   plan: { type: agent, title: Plan, config: { prompt: "..." } }
//! edges:
//!   - { id: e1, from: start.out, to: plan.in }
//! layout:
//!   viewport: { x: 0, y: 0, zoom: 1 }
//!   nodes: { start: { x: 40, y: 200 }, plan: { x: 240, y: 200 } }
//! ```

use std::fmt;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Version of the graph file format this build reads and writes.
pub const GRAPH_FORMAT: u32 = 1;

/// A whole graph file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphDoc {
    #[serde(rename = "carbon_graph")]
    pub format: u32,
    /// `<extension id>/<graph id>`, e.g. `carbon.agents/flow`.
    pub kind: String,
    pub name: String,
    /// Graph-level settings and any keys this build does not know, kept as
    /// they are (`inputs`, `budget`, ...).
    #[serde(flatten)]
    pub meta: IndexMap<String, Value>,
    #[serde(default)]
    pub nodes: IndexMap<String, Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub layout: Layout,
}

/// One node: its type, an optional title and the values of its fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub config: IndexMap<String, Value>,
}

impl Node {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            title: None,
            disabled: false,
            config: IndexMap::new(),
        }
    }
}

/// An arrow from an output port to an input port.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from: PortRef,
    pub to: PortRef,
}

/// `node.port`, written as one string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PortRef {
    pub node: String,
    pub port: String,
}

impl PortRef {
    pub fn new(node: impl Into<String>, port: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            port: port.into(),
        }
    }
}

impl fmt::Display for PortRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.node, self.port)
    }
}

impl Serialize for PortRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PortRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let (node, port) = text
            .split_once('.')
            .filter(|(node, port)| !node.is_empty() && !port.is_empty() && !port.contains('.'))
            .ok_or_else(|| serde::de::Error::custom(format!("`{text}` is not `node.port`")))?;
        Ok(Self::new(node, port))
    }
}

/// Where things are on the canvas; not part of the logic.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default)]
    pub viewport: Viewport,
    #[serde(default)]
    pub nodes: IndexMap<String, Pos>,
}

/// The visible part of the canvas: graph point at the top-left, and zoom.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            zoom: 1.,
        }
    }
}

/// A node's top-left corner in graph units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pos {
    pub x: f64,
    pub y: f64,
}

impl Pos {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

fn is_false(value: &bool) -> bool {
    !value
}

/// Node, port and edge ids: lowercase letters, digits, `-` and `_`, starting
/// with a letter (`agent-2`, `fix_core`).
pub fn check_graph_id(id: &str) -> Result<(), String> {
    let mut chars = id.chars();
    let ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "id `{id}` must start with a lowercase letter and use only lowercase letters, digits, `-` and `_`"
        ))
    }
}

impl GraphDoc {
    /// An empty graph of `kind`.
    pub fn new(kind: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            format: GRAPH_FORMAT,
            kind: kind.into(),
            name: name.into(),
            meta: IndexMap::new(),
            nodes: IndexMap::new(),
            edges: Vec::new(),
            layout: Layout::default(),
        }
    }

    /// A free node id for a node of `kind`: `<kind>`, else `<kind>-2`,
    /// `<kind>-3`, ... (the type name made into an id).
    pub fn next_node_id(&self, kind: &str) -> String {
        let base: String = kind
            .chars()
            .map(|c| match c {
                'a'..='z' | '0'..='9' | '-' | '_' => c,
                'A'..='Z' => c.to_ascii_lowercase(),
                _ => '-',
            })
            .collect();
        let base = if base.starts_with(|c: char| c.is_ascii_lowercase()) {
            base
        } else {
            format!("n{base}")
        };
        if !self.nodes.contains_key(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !self.nodes.contains_key(id))
            .expect("some number is free")
    }

    /// A free edge id: `e1`, `e2`, ...
    pub fn next_edge_id(&self) -> String {
        (1..)
            .map(|n| format!("e{n}"))
            .find(|id| self.edges.iter().all(|edge| &edge.id != id))
            .expect("some number is free")
    }

    /// The edges touching `node`.
    pub fn edges_of<'a>(&'a self, node: &'a str) -> impl Iterator<Item = &'a Edge> + 'a {
        self.edges
            .iter()
            .filter(move |edge| edge.from.node == node || edge.to.node == node)
    }

    pub fn edge(&self, id: &str) -> Option<&Edge> {
        self.edges.iter().find(|edge| edge.id == id)
    }

    /// Where `node` is drawn (the origin when the layout does not say).
    pub fn pos(&self, node: &str) -> Pos {
        self.layout.nodes.get(node).copied().unwrap_or_default()
    }
}
