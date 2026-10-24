//! What a run of a graph looks like on the canvas: each node's state, a short
//! badge (`$0.12`, `2/3`), its output and log lines. Set by the extension
//! running the graph (extension API v0.2 `graphs`).

use std::collections::BTreeMap;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeState {
    #[default]
    Idle,
    Queued,
    Running,
    /// Waiting for the user (an approval, a permission).
    Waiting,
    Done,
    Failed,
    Skipped,
    Cancelled,
}

impl NodeState {
    /// The word shown on the node (state is never colour alone).
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Waiting => "Waiting",
            Self::Done => "Done",
            Self::Failed => "Failed",
            Self::Skipped => "Skipped",
            Self::Cancelled => "Cancelled",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Waiting)
    }
}

/// One node in one run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeRun {
    pub state: NodeState,
    pub badge: Option<String>,
    /// Markdown shown in the inspector's Output section.
    pub output: Option<String>,
    pub changed: Option<SystemTime>,
}

/// One run of one graph file.
#[derive(Clone, Debug, PartialEq)]
pub struct RunState {
    pub id: u64,
    pub started: SystemTime,
    /// `None` while running; the error when it failed.
    pub outcome: Option<Result<(), String>>,
    pub nodes: BTreeMap<String, NodeRun>,
    /// `(node, line)` in order.
    pub log: Vec<(Option<String>, String)>,
}

impl RunState {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            started: SystemTime::now(),
            outcome: None,
            nodes: BTreeMap::new(),
            log: Vec::new(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }

    pub fn set_state(&mut self, node: &str, state: NodeState, badge: Option<String>) {
        let run = self.nodes.entry(node.to_string()).or_default();
        run.state = state;
        if badge.is_some() {
            run.badge = badge;
        }
        run.changed = Some(SystemTime::now());
    }

    pub fn set_output(&mut self, node: &str, markdown: String) {
        self.nodes.entry(node.to_string()).or_default().output = Some(markdown);
    }

    pub fn state(&self, node: &str) -> NodeState {
        self.nodes
            .get(node)
            .map(|run| run.state)
            .unwrap_or_default()
    }

    /// Ends the run; nodes still active become cancelled (or failed).
    pub fn finish(&mut self, outcome: Result<(), String>) {
        let end = if outcome.is_ok() {
            NodeState::Cancelled
        } else {
            NodeState::Failed
        };
        for run in self.nodes.values_mut() {
            if run.state.is_active() {
                run.state = end;
            }
        }
        self.outcome = Some(outcome);
    }
}
