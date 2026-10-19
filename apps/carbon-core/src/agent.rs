//! Agent sessions: data model only. How sessions talk to agents (ACP, direct
//! model APIs) is not decided yet (`project/not_built/README.md`, Pillar 3).

use std::collections::BTreeMap;

use crate::id::define_id;

define_id!(
    /// One agent conversation.
    AgentSessionId
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Running,
    /// Paused until the user approves an action.
    AwaitingApproval,
    Finished,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Agent,
    System,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct AgentSession {
    id: AgentSessionId,
    pub title: String,
    status: AgentStatus,
    messages: Vec<Message>,
}

impl AgentSession {
    pub fn id(&self) -> AgentSessionId {
        self.id
    }

    pub fn status(&self) -> &AgentStatus {
        &self.status
    }

    pub fn set_status(&mut self, status: AgentStatus) {
        self.status = status;
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn push(&mut self, role: Role, text: impl Into<String>) {
        self.messages.push(Message {
            role,
            text: text.into(),
        });
    }
}

/// All agent sessions, listed in creation order.
#[derive(Default)]
pub struct AgentStore {
    next_id: u64,
    sessions: BTreeMap<AgentSessionId, AgentSession>,
}

impl AgentStore {
    pub fn create(&mut self, title: impl Into<String>) -> AgentSessionId {
        self.next_id += 1;
        let id = AgentSessionId(self.next_id);
        self.sessions.insert(
            id,
            AgentSession {
                id,
                title: title.into(),
                status: AgentStatus::Idle,
                messages: Vec::new(),
            },
        );
        id
    }

    pub fn get(&self, id: AgentSessionId) -> Option<&AgentSession> {
        self.sessions.get(&id)
    }

    pub fn get_mut(&mut self, id: AgentSessionId) -> Option<&mut AgentSession> {
        self.sessions.get_mut(&id)
    }

    pub fn list(&self) -> impl Iterator<Item = &AgentSession> {
        self.sessions.values()
    }

    pub fn remove(&mut self, id: AgentSessionId) -> Option<AgentSession> {
        self.sessions.remove(&id)
    }
}
