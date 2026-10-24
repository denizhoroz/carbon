//! What a session showed: the agent's text, tool calls, permission decisions
//! and notes, in order, for the run view. Bounded, oldest dropped first.

/// Most items kept per session.
#[derive(Clone, Copy, Debug)]
pub struct TranscriptLimit(pub usize);

impl Default for TranscriptLimit {
    fn default() -> Self {
        Self(2000)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptItem {
    /// What Carbon sent.
    Prompt(String),
    /// The agent's text (chunks of one message are joined).
    Agent(String),
    Tool {
        id: String,
        title: String,
        status: String,
    },
    /// A permission request and what was answered.
    Permission { title: String, answer: String },
    /// Carbon's own notes (started, failed, cancelled).
    Note(String),
}

#[derive(Debug, Default)]
pub(super) struct Transcript {
    pub items: Vec<TranscriptItem>,
    limit: TranscriptLimit,
}

impl Transcript {
    pub fn push(&mut self, item: TranscriptItem) {
        // Text chunks grow the last agent item.
        if let (TranscriptItem::Agent(more), Some(TranscriptItem::Agent(text))) =
            (&item, self.items.last_mut())
        {
            text.push_str(more);
            return;
        }
        // A tool call's updates replace its earlier state.
        if let TranscriptItem::Tool { id, title, status } = &item
            && let Some(TranscriptItem::Tool {
                title: old_title,
                status: old_status,
                ..
            }) = self
                .items
                .iter_mut()
                .rev()
                .find(|i| matches!(i, TranscriptItem::Tool { id: old, .. } if old == id))
        {
            if !title.is_empty() {
                *old_title = title.clone();
            }
            *old_status = status.clone();
            return;
        }
        self.items.push(item);
        if self.items.len() > self.limit.0 {
            let extra = self.items.len() - self.limit.0;
            self.items.drain(..extra);
        }
    }
}
