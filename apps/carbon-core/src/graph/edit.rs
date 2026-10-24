//! Edits to a graph, each with its exact inverse, so undo and redo replay
//! them instead of keeping copies of the document.

use serde_json::Value;

use super::{Edge, GraphDoc, Node, Pos};

#[derive(Clone, Debug, PartialEq)]
pub enum GraphEdit {
    /// Adds a node at `index` in the node order (the end when larger).
    AddNode {
        id: String,
        node: Node,
        pos: Pos,
        index: usize,
    },
    /// Removes a node; `edges` are its arrows, removed with it (with their
    /// places in the edge list, for undo).
    RemoveNode {
        id: String,
        node: Node,
        pos: Pos,
        index: usize,
        edges: Vec<(usize, Edge)>,
    },
    /// Moves nodes: `(id, from, to)`.
    MoveNodes(Vec<(String, Pos, Pos)>),
    /// Sets (or with `None` removes) a config value.
    SetField {
        node: String,
        key: String,
        old: Option<Value>,
        new: Option<Value>,
    },
    SetTitle {
        node: String,
        old: Option<String>,
        new: Option<String>,
    },
    SetDisabled {
        node: String,
        disabled: bool,
    },
    /// Sets (or removes) a graph-level value.
    SetMeta {
        key: String,
        old: Option<Value>,
        new: Option<Value>,
    },
    Rename {
        old: String,
        new: String,
    },
    /// The graph's own name.
    SetName {
        old: String,
        new: String,
    },
    AddEdge {
        edge: Edge,
        index: usize,
    },
    RemoveEdge {
        edge: Edge,
        index: usize,
    },
    /// Several edits as one step.
    Batch(Vec<GraphEdit>),
}

impl GraphEdit {
    /// Removing `id` from `doc`, with its arrows (`None` when it is not there).
    pub fn remove_node(doc: &GraphDoc, id: &str) -> Option<Self> {
        let index = doc.nodes.get_index_of(id)?;
        let edges = doc
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.from.node == id || edge.to.node == id)
            .map(|(ix, edge)| (ix, edge.clone()))
            .collect();
        Some(Self::RemoveNode {
            id: id.to_string(),
            node: doc.nodes[index].clone(),
            pos: doc.pos(id),
            index,
            edges,
        })
    }

    /// Removing the arrow `id`.
    pub fn remove_edge(doc: &GraphDoc, id: &str) -> Option<Self> {
        let index = doc.edges.iter().position(|edge| edge.id == id)?;
        Some(Self::RemoveEdge {
            edge: doc.edges[index].clone(),
            index,
        })
    }

    /// Setting a config value, recording the old one.
    pub fn set_field(doc: &GraphDoc, node: &str, key: &str, new: Option<Value>) -> Option<Self> {
        let old = doc.nodes.get(node)?.config.get(key).cloned();
        (old != new).then(|| Self::SetField {
            node: node.to_string(),
            key: key.to_string(),
            old,
            new,
        })
    }

    /// The edit that undoes this one.
    pub fn inverse(&self) -> Self {
        match self {
            Self::AddNode {
                id,
                node,
                pos,
                index,
            } => Self::RemoveNode {
                id: id.clone(),
                node: node.clone(),
                pos: *pos,
                index: *index,
                edges: Vec::new(),
            },
            Self::RemoveNode {
                id,
                node,
                pos,
                index,
                edges,
            } => {
                let mut batch = vec![Self::AddNode {
                    id: id.clone(),
                    node: node.clone(),
                    pos: *pos,
                    index: *index,
                }];
                batch.extend(edges.iter().map(|(index, edge)| Self::AddEdge {
                    edge: edge.clone(),
                    index: *index,
                }));
                Self::Batch(batch)
            }
            Self::MoveNodes(moves) => Self::MoveNodes(
                moves
                    .iter()
                    .map(|(id, from, to)| (id.clone(), *to, *from))
                    .collect(),
            ),
            Self::SetField {
                node,
                key,
                old,
                new,
            } => Self::SetField {
                node: node.clone(),
                key: key.clone(),
                old: new.clone(),
                new: old.clone(),
            },
            Self::SetTitle { node, old, new } => Self::SetTitle {
                node: node.clone(),
                old: new.clone(),
                new: old.clone(),
            },
            Self::SetDisabled { node, disabled } => Self::SetDisabled {
                node: node.clone(),
                disabled: !disabled,
            },
            Self::SetMeta { key, old, new } => Self::SetMeta {
                key: key.clone(),
                old: new.clone(),
                new: old.clone(),
            },
            Self::Rename { old, new } => Self::Rename {
                old: new.clone(),
                new: old.clone(),
            },
            Self::SetName { old, new } => Self::SetName {
                old: new.clone(),
                new: old.clone(),
            },
            Self::AddEdge { edge, index } => Self::RemoveEdge {
                edge: edge.clone(),
                index: *index,
            },
            Self::RemoveEdge { edge, index } => Self::AddEdge {
                edge: edge.clone(),
                index: *index,
            },
            Self::Batch(edits) => Self::Batch(edits.iter().rev().map(Self::inverse).collect()),
        }
    }

    /// Applies the edit. It must fit the document (made from it, or its
    /// inverse applied right after it); a stale edit is skipped where it
    /// does not fit.
    pub fn apply(&self, doc: &mut GraphDoc) {
        match self {
            Self::AddNode {
                id,
                node,
                pos,
                index,
            } => {
                let index = (*index).min(doc.nodes.len());
                doc.nodes.shift_insert(index, id.clone(), node.clone());
                doc.layout.nodes.insert(id.clone(), *pos);
            }
            Self::RemoveNode { id, edges, .. } => {
                // Arrows first, from the back so the recorded places hold.
                for (index, edge) in edges.iter().rev() {
                    if doc.edges.get(*index).is_some_and(|at| at == edge) {
                        doc.edges.remove(*index);
                    } else {
                        doc.edges.retain(|at| at != edge);
                    }
                }
                doc.nodes.shift_remove(id);
                doc.layout.nodes.shift_remove(id);
            }
            Self::MoveNodes(moves) => {
                for (id, _, to) in moves {
                    if doc.nodes.contains_key(id) {
                        doc.layout.nodes.insert(id.clone(), *to);
                    }
                }
            }
            Self::SetField { node, key, new, .. } => {
                if let Some(node) = doc.nodes.get_mut(node) {
                    match new {
                        Some(value) => {
                            node.config.insert(key.clone(), value.clone());
                        }
                        None => {
                            node.config.shift_remove(key);
                        }
                    }
                }
            }
            Self::SetTitle { node, new, .. } => {
                if let Some(node) = doc.nodes.get_mut(node) {
                    node.title = new.clone();
                }
            }
            Self::SetDisabled { node, disabled } => {
                if let Some(node) = doc.nodes.get_mut(node) {
                    node.disabled = *disabled;
                }
            }
            Self::SetMeta { key, new, .. } => match new {
                Some(value) => {
                    doc.meta.insert(key.clone(), value.clone());
                }
                None => {
                    doc.meta.shift_remove(key);
                }
            },
            Self::Rename { old, new } => {
                if let Some(index) = doc.nodes.get_index_of(old) {
                    let node = doc.nodes.shift_remove(old).expect("present");
                    doc.nodes.shift_insert(index, new.clone(), node);
                    if let Some(index) = doc.layout.nodes.get_index_of(old) {
                        let pos = doc.layout.nodes.shift_remove(old).expect("present");
                        doc.layout.nodes.shift_insert(index, new.clone(), pos);
                    }
                    for edge in &mut doc.edges {
                        for end in [&mut edge.from, &mut edge.to] {
                            if &end.node == old {
                                end.node = new.clone();
                            }
                        }
                    }
                }
            }
            Self::SetName { new, .. } => doc.name = new.clone(),
            Self::AddEdge { edge, index } => {
                let index = (*index).min(doc.edges.len());
                doc.edges.insert(index, edge.clone());
            }
            Self::RemoveEdge { edge, index } => {
                if doc.edges.get(*index).is_some_and(|at| at == edge) {
                    doc.edges.remove(*index);
                } else {
                    doc.edges.retain(|at| at != edge);
                }
            }
            Self::Batch(edits) => {
                for edit in edits {
                    edit.apply(doc);
                }
            }
        }
    }

    /// Whether this edit changes the logic (not only where nodes are drawn).
    pub fn changes_logic(&self) -> bool {
        match self {
            Self::MoveNodes(_) => false,
            Self::Batch(edits) => edits.iter().any(Self::changes_logic),
            _ => true,
        }
    }
}
