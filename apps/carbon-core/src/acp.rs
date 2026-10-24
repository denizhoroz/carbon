//! Agents over ACP (Agent Client Protocol): agent programs such as Claude
//! Code (through its adapter) spoken to with JSON-RPC, one message per line,
//! over stdin/stdout (`.agents/documents/agents.md` s4).
//!
//! The [`AcpManager`] starts sessions and runs prompts without blocking the
//! caller: each session has its own thread and process. What happens comes
//! back as [`AcpEvent`]s on one channel, tagged with the session and its
//! owner (the extension that started it). Carbon alone answers an agent's
//! permission requests (`answer_permission`); the agent's own mode does the
//! rest (`PermissionMode`).
//!
//! Only the parts of ACP Carbon uses are typed here, so the protocol's churn
//! stays in this module.

mod manager;
mod resolve;
mod session;
mod transcript;

pub use manager::{AcpManager, SessionId, SessionOptions};
pub use resolve::{AgentProgram, Launch, find_on_path, npm_dir};
pub use transcript::{TranscriptItem, TranscriptLimit};

/// How freely an agent may act without asking (mapped onto the agent's
/// own modes, e.g. Claude Code's `default`, `acceptEdits`,
/// `bypassPermissions`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionMode {
    /// Every tool that needs permission asks.
    #[default]
    Ask,
    /// File edits in the folder go ahead; other things ask.
    AllowEdits,
    /// Nothing asks.
    AllowAll,
}

impl PermissionMode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "ask" => Some(Self::Ask),
            "allow-edits" => Some(Self::AllowEdits),
            "allow-all" => Some(Self::AllowAll),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::AllowEdits => "allow-edits",
            Self::AllowAll => "allow-all",
        }
    }

    /// The agent modes that mean this, best first.
    fn agent_modes(self) -> &'static [&'static str] {
        match self {
            Self::Ask => &["default"],
            Self::AllowEdits => &["acceptEdits"],
            Self::AllowAll => &["bypassPermissions", "auto"],
        }
    }
}

/// Why a session could not start or stopped working.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// The agent wants you to sign in first.
    AuthRequired,
    /// The program is not there and cannot be installed.
    MissingProgram,
    /// The program stopped.
    Crashed,
    /// It answered something Carbon does not understand.
    Protocol,
}

impl FailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth-required",
            Self::MissingProgram => "missing-program",
            Self::Crashed => "crashed",
            Self::Protocol => "protocol",
        }
    }
}

/// Tokens and cost a session reported.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Cache reads and writes, counted apart by some agents.
    pub cached_tokens: u64,
    /// In USD, when the agent says.
    pub cost_usd: Option<f64>,
}

/// A choice offered with a permission request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionOption {
    pub id: String,
    pub name: String,
    /// `allow_once`, `allow_always`, `reject_once`, `reject_always`.
    pub kind: String,
}

/// What a session did, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum AcpEvent {
    /// Finding, installing or starting the program.
    Starting {
        session: SessionId,
        owner: String,
        message: String,
    },
    /// Ready for prompts.
    Ready { session: SessionId, owner: String },
    /// It could not start, or stopped.
    Failed {
        session: SessionId,
        owner: String,
        kind: FailureKind,
        message: String,
    },
    /// Part of the agent's answer.
    Text {
        session: SessionId,
        owner: String,
        text: String,
    },
    /// A tool call started or changed state.
    ToolCall {
        session: SessionId,
        owner: String,
        id: String,
        title: String,
        status: String,
    },
    /// Usage so far in the session.
    Usage {
        session: SessionId,
        owner: String,
        usage: Usage,
    },
    /// The agent asks before doing something; answer with
    /// [`AcpManager::answer_permission`].
    Permission {
        session: SessionId,
        owner: String,
        request: u64,
        title: String,
        kind: String,
        options: Vec<PermissionOption>,
    },
    /// A prompt ended: why, the agent's whole answer, and usage.
    PromptFinished {
        session: SessionId,
        owner: String,
        stop_reason: String,
        text: String,
        truncated: bool,
        usage: Option<Usage>,
    },
    /// The session ended (closed by its owner or by Carbon).
    Closed { session: SessionId, owner: String },
}

impl AcpEvent {
    pub fn session(&self) -> SessionId {
        match self {
            Self::Starting { session, .. }
            | Self::Ready { session, .. }
            | Self::Failed { session, .. }
            | Self::Text { session, .. }
            | Self::ToolCall { session, .. }
            | Self::Usage { session, .. }
            | Self::Permission { session, .. }
            | Self::PromptFinished { session, .. }
            | Self::Closed { session, .. } => *session,
        }
    }

    pub fn owner(&self) -> &str {
        match self {
            Self::Starting { owner, .. }
            | Self::Ready { owner, .. }
            | Self::Failed { owner, .. }
            | Self::Text { owner, .. }
            | Self::ToolCall { owner, .. }
            | Self::Usage { owner, .. }
            | Self::Permission { owner, .. }
            | Self::PromptFinished { owner, .. }
            | Self::Closed { owner, .. } => owner,
        }
    }
}
