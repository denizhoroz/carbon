use carbon_core::{AgentStatus, AgentStore, Role};

#[test]
fn sessions_keep_messages_status_and_order() {
    let mut store = AgentStore::default();
    let first = store.create("Refactor parser");
    let second = store.create("Write tests");

    let session = store.get_mut(first).unwrap();
    assert_eq!(session.status(), &AgentStatus::Idle);
    session.push(Role::User, "Split parse() into two functions");
    session.set_status(AgentStatus::Running);
    session.push(Role::Agent, "Working on it");

    let session = store.get(first).unwrap();
    assert_eq!(session.messages().len(), 2);
    assert_eq!(session.messages()[1].role, Role::Agent);
    assert_eq!(session.status(), &AgentStatus::Running);

    let titles: Vec<&str> = store.list().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Refactor parser", "Write tests"]);

    assert!(store.remove(second).is_some());
    assert!(store.get(second).is_none());
}
