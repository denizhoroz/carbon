//! What an extension declares about its graphs: the graph kind (`[[graphs]]`)
//! and its node types (`[[node_types]]`) with ports and inspector fields.
//! Carbon draws the palette and inspector and runs the generic checks from
//! these, without calling the extension's code.
//!
//! ```toml
//! [[node_types]]
//! graph = "flow"
//! id = "agent"
//! title = "Agent"
//! inputs = [ { id = "in", label = "In", multi = true } ]
//! outputs = [ { id = "out", label = "Out", multi = true } ]
//! [[node_types.fields]]
//! key = "prompt"
//! label = "Prompt"
//! kind = "text"
//! multiline = true
//! required = true
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Node;

/// The extra output of a node whose outputs come from a list of routes.
pub const ROUTES_ELSE: &str = "else";

/// A graph kind: where its files live and what the panel offers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphSpec {
    /// Unique within the extension; the file's `kind` is `<extension>/<id>`.
    pub id: String,
    pub name: String,
    /// Folder in the project for new files (`.carbon/flows`).
    pub dir: String,
    /// File name ending without the dot (`flow.yaml`).
    pub suffix: String,
    /// Package file used as the start of a new graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// The extension's code checks graphs too (`check-graph`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub check: bool,
    /// Buttons in the panel's top bar; they send the extension an event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<GraphAction>,
    /// Graph-level fields (shown when nothing is selected), stored in the
    /// file's top level by dotted key (`budget.max_cost_usd`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldSpec>,
}

/// A top-bar button of a graph panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphAction {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub primary: bool,
    /// `idle` (no run of this graph going), `running`, or always when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

/// A node type of one graph kind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeTypeSpec {
    /// The `[[graphs]]` id it belongs to.
    pub graph: String,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Palette group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// At most this many in one graph (Start: 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<PortSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<PortSpec>,
    /// A `list` field whose items (`{ id, ... }`) each make an output port,
    /// plus `else`; added after `outputs` (a router).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs_from: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldSpec>,
}

/// A port: where arrows start (outputs) or end (inputs).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortSpec {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// More than one arrow may use it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub multi: bool,
    /// An input that arrows may reach backwards (a loop's way back): the
    /// only place a graph may have a cycle.
    #[serde(default, skip_serializing_if = "is_false")]
    pub loop_back: bool,
}

impl PortSpec {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: String::new(),
            multi: false,
            loop_back: false,
        }
    }

    /// The label, else the id.
    pub fn label(&self) -> &str {
        if self.label.is_empty() {
            &self.id
        } else {
            &self.label
        }
    }
}

/// A field of the inspector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSpec {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<toml::Value>,
    /// `choice`: `{ value, label }`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<FieldChoice>,
    /// `list`: the fields of each item (each item also has an `id`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_fields: Vec<FieldSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub multiline: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Bool,
    Text,
    Integer,
    Number,
    Choice,
    List,
    /// An agent from the agent registry (`[[agents]]`).
    Agent,
    /// A path in the project.
    File,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldChoice {
    pub value: String,
    pub label: String,
}

impl FieldSpec {
    /// The default as JSON (what a new node's config starts with).
    pub fn default_json(&self) -> Option<Value> {
        self.default
            .as_ref()
            .and_then(|value| serde_json::to_value(value).ok())
    }
}

impl NodeTypeSpec {
    /// A new node of this type with its fields' defaults.
    pub fn new_node(&self) -> Node {
        let mut node = Node::new(self.id.clone());
        for field in &self.fields {
            if let Some(value) = field.default_json() {
                node.config.insert(field.key.clone(), value);
            }
        }
        node
    }

    /// The output ports of `node`: the declared ones, then one per route
    /// (`outputs_from`) and `else`.
    pub fn outputs_for(&self, node: &Node) -> Vec<PortSpec> {
        let mut ports = self.outputs.clone();
        if let Some(key) = &self.outputs_from {
            let routes = node.config.get(key).and_then(Value::as_array);
            for route in routes.into_iter().flatten() {
                if let Some(id) = route.get("id").and_then(Value::as_str) {
                    let label = route
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or(id)
                        .to_string();
                    ports.push(PortSpec {
                        id: id.to_string(),
                        label,
                        multi: true,
                        loop_back: false,
                    });
                }
            }
            ports.push(PortSpec {
                id: ROUTES_ELSE.into(),
                label: "Else".into(),
                multi: true,
                loop_back: false,
            });
        }
        ports
    }

    pub fn input(&self, id: &str) -> Option<&PortSpec> {
        self.inputs.iter().find(|port| port.id == id)
    }
}

fn is_false(value: &bool) -> bool {
    !value
}
