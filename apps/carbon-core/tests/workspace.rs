use carbon_core::{Core, CoreError, Workspace};

#[test]
fn opens_a_folder_and_names_it() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("my-project");
    std::fs::create_dir(&project).unwrap();

    let workspace = Workspace::open(&project).unwrap();
    assert_eq!(workspace.name(), "my-project");
    assert!(workspace.root().is_absolute());
}

#[test]
fn refuses_a_file_or_missing_path() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "x").unwrap();

    assert!(matches!(
        Workspace::open(&file),
        Err(CoreError::NotADirectory(_))
    ));
    assert!(matches!(
        Workspace::open(dir.path().join("missing")),
        Err(CoreError::Io { .. })
    ));
}

#[test]
fn resolves_inside_and_blocks_escapes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    let workspace = Workspace::open(dir.path()).unwrap();

    let inside = workspace.resolve("src").unwrap();
    assert_eq!(inside, workspace.root().join("src"));
    // Not yet existing (a new file) still resolves.
    assert_eq!(
        workspace.resolve("src/new.rs").unwrap(),
        workspace.root().join("src/new.rs")
    );
    // `..` that stays inside is fine; leaving the root is not.
    assert_eq!(workspace.resolve("src/../src").unwrap(), inside);
    assert!(matches!(
        workspace.resolve("../outside"),
        Err(CoreError::OutsideWorkspace(_))
    ));
    assert!(matches!(
        workspace.resolve(std::env::temp_dir()),
        Err(CoreError::OutsideWorkspace(_))
    ));
}

#[test]
fn core_needs_an_open_workspace() {
    let mut core = Core::new();
    assert!(matches!(core.list_dir("."), Err(CoreError::NoWorkspace)));
    assert!(matches!(
        core.open_file("a.txt"),
        Err(CoreError::NoWorkspace)
    ));

    let dir = tempfile::tempdir().unwrap();
    core.open_workspace(dir.path()).unwrap();
    assert!(core.workspace().is_some());
    assert!(core.list_dir(".").unwrap().is_empty());
}
