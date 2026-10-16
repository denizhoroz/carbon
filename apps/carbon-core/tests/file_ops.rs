use carbon_core::{Core, CoreError, Row};

fn names(core: &Core) -> Vec<String> {
    core.tree()
        .unwrap()
        .visible_rows()
        .into_iter()
        .map(|Row { entry, depth }| format!("{}@{depth}", entry.name))
        .collect()
}

/// src/ (main.rs), notes.txt
fn sample() -> (tempfile::TempDir, Core) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "hi").unwrap();
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();
    (dir, core)
}

#[test]
fn create_file_and_folder_show_in_the_tree() {
    let (dir, mut core) = sample();
    let file = core.create_file("src", "lib.rs").unwrap();
    assert!(file.is_file());
    assert_eq!(std::fs::read(&file).unwrap(), b"");
    let folder = core.create_dir("", "docs").unwrap();
    assert!(folder.is_dir());
    // `src` opened so the new file is visible without waiting for the watcher.
    assert_eq!(
        names(&core),
        ["docs@0", "src@0", "lib.rs@1", "main.rs@1", "notes.txt@0"]
    );
    drop(dir);
}

#[test]
fn create_refuses_bad_names_existing_paths_and_escapes() {
    let (_dir, mut core) = sample();
    for name in ["", "a/b", "a\\b", "what?", "..", "trailing."] {
        assert!(
            matches!(core.create_file("", name), Err(CoreError::InvalidName(_))),
            "{name:?}"
        );
    }
    assert!(matches!(
        core.create_file("", "notes.txt"),
        Err(CoreError::AlreadyExists(_))
    ));
    assert!(matches!(
        core.create_dir("", "src"),
        Err(CoreError::AlreadyExists(_))
    ));
    assert!(matches!(
        core.create_file("..", "escape.txt"),
        Err(CoreError::OutsideWorkspace(_))
    ));
}

#[test]
fn rename_moves_entries_and_open_buffers_follow() {
    let (_dir, mut core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    core.open_file("src/main.rs").unwrap();

    let renamed = core.rename(root.join("src"), "app").unwrap();
    assert_eq!(renamed, root.join("app"));
    assert!(root.join("app/main.rs").is_file() && !root.join("src").exists());
    assert_eq!(
        core.active_buffer().unwrap().path(),
        Some(root.join("app/main.rs").as_path())
    );
    assert_eq!(names(&core), ["app@0", "notes.txt@0"]);

    assert!(matches!(
        core.rename(root.join("notes.txt"), "app"),
        Err(CoreError::AlreadyExists(_))
    ));
    assert!(matches!(
        core.rename(&root, "other"),
        Err(CoreError::WorkspaceRoot)
    ));
    // Same name is a no-op; a case-only change works everywhere.
    assert_eq!(
        core.rename(root.join("notes.txt"), "notes.txt").unwrap(),
        root.join("notes.txt")
    );
    core.rename(root.join("notes.txt"), "Notes.txt").unwrap();
    let listed: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(listed.contains(&"Notes.txt".to_string()), "{listed:?}");
}

#[test]
fn trash_refuses_the_root_and_outside_paths() {
    let (dir, mut core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    assert!(matches!(core.trash(&root), Err(CoreError::WorkspaceRoot)));
    let outside = dir.path().parent().unwrap().join("outside.txt");
    assert!(core.trash(outside).is_err());
}

/// Moves a real file to this machine's Recycle Bin / Trash, so it only runs
/// on request: `cargo test -p carbon-core --test file_ops -- --ignored`.
#[test]
#[ignore]
fn trash_moves_a_file_out_of_the_workspace() {
    let (_dir, mut core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    core.trash(root.join("notes.txt")).unwrap();
    assert!(!root.join("notes.txt").exists());
    assert_eq!(names(&core), ["src@0"]);
}
