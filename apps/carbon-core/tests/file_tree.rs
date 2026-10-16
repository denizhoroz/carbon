use carbon_core::{Core, CoreError, EntryKind, list_dir};

fn names(entries: &[carbon_core::FileEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn folders_first_then_case_insensitive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for file in ["b.txt", "A.txt", "c.txt"] {
        std::fs::write(root.join(file), "").unwrap();
    }
    for folder in ["zeta", "Alpha"] {
        std::fs::create_dir(root.join(folder)).unwrap();
    }

    let entries = list_dir(root).unwrap();
    assert_eq!(names(&entries), ["Alpha", "zeta", "A.txt", "b.txt", "c.txt"]);
    assert_eq!(entries[0].kind, EntryKind::Dir);
    assert_eq!(entries[2].kind, EntryKind::File);
}

#[test]
fn hides_vcs_folders_and_marks_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::create_dir(root.join("target")).unwrap();
    std::fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
    std::fs::write(root.join("debug.log"), "").unwrap();
    std::fs::write(root.join("main.rs"), "").unwrap();
    std::fs::write(root.join(".env"), "").unwrap();

    let entries = list_dir(root).unwrap();
    assert_eq!(names(&entries), ["target", ".env", ".gitignore", "debug.log", "main.rs"]);
    let ignored: Vec<&str> = entries.iter().filter(|e| e.ignored).map(|e| e.name.as_str()).collect();
    // Dotfiles are shown normally; only .gitignore rules dim entries.
    assert_eq!(ignored, ["target", "debug.log"]);
}

#[test]
fn parent_ignore_rules_apply_in_subfolders() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join(".gitignore"), "*.tmp\n").unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join("src/keep.rs"), "").unwrap();
    std::fs::write(root.join("src/scratch.tmp"), "").unwrap();

    let entries = list_dir(&root.join("src")).unwrap();
    let scratch = entries.iter().find(|e| e.name == "scratch.tmp").unwrap();
    let keep = entries.iter().find(|e| e.name == "keep.rs").unwrap();
    assert!(scratch.ignored);
    assert!(!keep.ignored);
}

#[test]
fn core_lists_only_inside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "").unwrap();

    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();
    assert_eq!(names(&core.list_dir("src").unwrap()), ["lib.rs"]);
    assert!(matches!(core.list_dir(".."), Err(CoreError::OutsideWorkspace(_))));
    assert!(matches!(core.list_dir("src/lib.rs"), Err(CoreError::NotADirectory(_))));
}
