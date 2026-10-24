//! Undo and redo for one graph document. Typing into one field merges into
//! one step while it keeps coming (`MERGE_WINDOW`); a drag is pushed once,
//! when it ends.

use std::time::{Duration, Instant};

use super::{GraphDoc, GraphEdit};

/// Edits with the same merge key closer together than this are one step.
pub const MERGE_WINDOW: Duration = Duration::from_secs(1);

/// Most steps kept.
const LIMIT: usize = 500;

struct Step {
    edit: GraphEdit,
    merge_key: Option<String>,
    at: Instant,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<GraphEdit>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `edit` to `doc` and records it. With a `merge_key` equal to the
    /// last step's, within `MERGE_WINDOW`, the two become one step.
    pub fn apply(&mut self, doc: &mut GraphDoc, edit: GraphEdit, merge_key: Option<String>) {
        self.apply_at(doc, edit, merge_key, Instant::now());
    }

    /// `apply` with the time given (tests).
    pub fn apply_at(
        &mut self,
        doc: &mut GraphDoc,
        edit: GraphEdit,
        merge_key: Option<String>,
        at: Instant,
    ) {
        edit.apply(doc);
        self.redo.clear();
        if let (Some(key), Some(last)) = (&merge_key, self.undo.last_mut())
            && last.merge_key.as_ref() == Some(key)
            && at.duration_since(last.at) <= MERGE_WINDOW
        {
            last.edit = merge(&last.edit, edit);
            last.at = at;
            return;
        }
        self.undo.push(Step {
            edit,
            merge_key,
            at,
        });
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// Records an edit already applied (a drag, applied live while dragging).
    pub fn record(&mut self, edit: GraphEdit) {
        self.redo.clear();
        self.undo.push(Step {
            edit,
            merge_key: None,
            at: Instant::now(),
        });
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    /// Undoes the last step; false when there is none.
    pub fn undo(&mut self, doc: &mut GraphDoc) -> bool {
        let Some(step) = self.undo.pop() else {
            return false;
        };
        step.edit.inverse().apply(doc);
        self.redo.push(step.edit);
        true
    }

    /// Redoes the last undone step; false when there is none.
    pub fn redo(&mut self, doc: &mut GraphDoc) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        edit.apply(doc);
        self.undo.push(Step {
            edit,
            merge_key: None,
            at: Instant::now(),
        });
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

/// `first` then `second` as one edit: a field set twice keeps the first old
/// value and the last new one.
fn merge(first: &GraphEdit, second: GraphEdit) -> GraphEdit {
    match (first, second) {
        (
            GraphEdit::SetField { node, key, old, .. },
            GraphEdit::SetField {
                node: node2,
                key: key2,
                new,
                ..
            },
        ) if *node == node2 && *key == key2 => GraphEdit::SetField {
            node: node.clone(),
            key: key.clone(),
            old: old.clone(),
            new,
        },
        (
            GraphEdit::SetTitle { node, old, .. },
            GraphEdit::SetTitle {
                node: node2, new, ..
            },
        ) if *node == node2 => GraphEdit::SetTitle {
            node: node.clone(),
            old: old.clone(),
            new,
        },
        (GraphEdit::SetMeta { key, old, .. }, GraphEdit::SetMeta { key: key2, new, .. })
            if *key == key2 =>
        {
            GraphEdit::SetMeta {
                key: key.clone(),
                old: old.clone(),
                new,
            }
        }
        (GraphEdit::SetName { old, .. }, GraphEdit::SetName { new, .. }) => GraphEdit::SetName {
            old: old.clone(),
            new,
        },
        (first, second) => GraphEdit::Batch(vec![first.clone(), second]),
    }
}
