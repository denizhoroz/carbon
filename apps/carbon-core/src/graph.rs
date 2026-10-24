//! Graphs: node-and-arrow documents drawn by the graph panel (agent flows of
//! the Agents extension; later the paradigm editor). Plain data with no UI
//! types (`.agents/documents/agents.md`).
//!
//! A graph is a YAML file in the project (`doc.rs`, `yaml.rs`). Extensions
//! declare its kind and node types (`spec.rs`); Carbon draws and edits it,
//! checks what any graph must satisfy (`validate.rs`), and keeps an undo
//! history of invertible edits (`edit.rs`, `history.rs`). What the nodes
//! mean is up to the extension.

mod doc;
mod edit;
mod history;
mod run_state;
mod spec;
mod validate;
mod yaml;

pub use doc::{Edge, GRAPH_FORMAT, GraphDoc, Layout, Node, PortRef, Pos, Viewport, check_graph_id};
pub use edit::GraphEdit;
pub use history::{History, MERGE_WINDOW};
pub use run_state::{NodeRun, NodeState, RunState};
pub use spec::{FieldKind, FieldSpec, GraphAction, GraphSpec, NodeTypeSpec, PortSpec, ROUTES_ELSE};
pub use validate::{Diagnostic, Severity, validate};
pub use yaml::{from_yaml, logic_hash, read_file, to_yaml, write_file};
