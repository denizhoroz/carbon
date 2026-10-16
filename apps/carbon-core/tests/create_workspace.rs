use carbon_core::{Core, CoreError, create_folder};

#[test]
fn creates_and_opens_a_new_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut core = Core::new();
    let workspace = core.create_workspace(dir.path(), "  new-project ").unwrap();
    assert_eq!(workspace.name(), "new-project");
    assert!(dir.path().join("new-project").is_dir());
    assert!(core.list_dir(".").unwrap().is_empty());
}

#[test]
fn rejects_bad_names() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["", "  ", ".", "..", "a/b", "a\\b", "c:", "what?", "trailing.", "tab\there"] {
        assert!(
            matches!(create_folder(dir.path(), name), Err(CoreError::InvalidName(_))),
            "{name:?} should be rejected"
        );
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn refuses_existing_names_and_missing_parents() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("taken")).unwrap();
    std::fs::write(dir.path().join("file"), "").unwrap();

    assert!(matches!(create_folder(dir.path(), "taken"), Err(CoreError::AlreadyExists(_))));
    assert!(matches!(create_folder(dir.path(), "file"), Err(CoreError::AlreadyExists(_))));
    assert!(matches!(
        create_folder(&dir.path().join("missing"), "x"),
        Err(CoreError::NotADirectory(_))
    ));
}

#[test]
fn close_workspace_keeps_open_buffers() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "x").unwrap();
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();
    let id = core.open_file("a.txt").unwrap();

    core.close_workspace();
    assert!(core.workspace().is_none());
    assert!(matches!(core.list_dir("."), Err(CoreError::NoWorkspace)));
    assert!(core.buffers.get(id).is_some());
}
