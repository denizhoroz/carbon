use std::path::Path;
use std::time::{Duration, Instant};

use carbon_core::{Core, CoreError, FileTree, FsChanges, Row};

/// "name@depth" for each visible row.
fn rows(tree: &FileTree) -> Vec<String> {
    tree.visible_rows()
        .iter()
        .map(|Row { entry, depth, .. }| format!("{}@{depth}", entry.name))
        .collect()
}

/// a/ (b/ (deep.txt), a.txt), z/, root.txt
fn sample() -> (tempfile::TempDir, Core) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::create_dir(root.join("z")).unwrap();
    std::fs::write(root.join("a/a.txt"), "").unwrap();
    std::fs::write(root.join("a/b/deep.txt"), "").unwrap();
    std::fs::write(root.join("root.txt"), "").unwrap();
    let mut core = Core::new();
    core.open_workspace(root).unwrap();
    (dir, core)
}

#[test]
fn rows_follow_expanded_folders_depth_first() {
    let (_dir, mut core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    let tree = core.tree_mut().unwrap();
    assert_eq!(rows(tree), ["a@0", "z@0", "root.txt@0"]);

    tree.expand(&root.join("a")).unwrap();
    tree.expand(&root.join("a/b")).unwrap();
    assert_eq!(
        rows(tree),
        ["a@0", "b@1", "deep.txt@2", "a.txt@1", "z@0", "root.txt@0"]
    );

    // Collapsing a parent hides its subtree; reopening shows `b` still open.
    tree.collapse(&root.join("a"));
    assert_eq!(rows(tree), ["a@0", "z@0", "root.txt@0"]);
    tree.toggle(&root.join("a")).unwrap();
    assert_eq!(rows(tree)[..3], ["a@0", "b@1", "deep.txt@2"]);

    tree.collapse_all();
    assert_eq!(rows(tree), ["a@0", "z@0", "root.txt@0"]);
    assert!(!tree.is_expanded(&root.join("a/b")));
}

#[test]
fn expand_refuses_paths_outside_and_files() {
    let (dir, mut core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    let tree = core.tree_mut().unwrap();
    let outside = dir.path().parent().unwrap().to_path_buf();
    assert!(matches!(
        tree.expand(&outside),
        Err(CoreError::OutsideWorkspace(_))
    ));
    assert!(matches!(
        tree.expand(&root.join("root.txt")),
        Err(CoreError::NotADirectory(_))
    ));
}

#[test]
fn refresh_picks_up_new_and_removed_entries() {
    let (_dir, mut core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    core.tree_mut().unwrap().expand(&root.join("a")).unwrap();
    core.tree_mut().unwrap().expand(&root.join("a/b")).unwrap();

    std::fs::write(root.join("new.txt"), "").unwrap();
    std::fs::remove_dir_all(root.join("a/b")).unwrap();
    let changes = FsChanges {
        dirs: vec![root.join("a/b"), root.clone(), root.join("a")],
        rescan: false,
        git: false,
    };
    assert!(core.apply_fs_changes(&changes).unwrap());

    let tree = core.tree().unwrap();
    assert_eq!(
        rows(tree),
        ["a@0", "a.txt@1", "z@0", "new.txt@0", "root.txt@0"]
    );
    assert!(!tree.is_loaded(&root.join("a/b")));
    // Unloaded folders are ignored; nothing changes the second time.
    assert!(!core.apply_fs_changes(&changes).unwrap());
}

#[test]
fn rescan_refreshes_every_loaded_folder() {
    let (_dir, mut core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    core.tree_mut().unwrap().expand(&root.join("a")).unwrap();
    std::fs::write(root.join("a/later.txt"), "").unwrap();
    let rescan = FsChanges {
        dirs: Vec::new(),
        rescan: true,
        git: false,
    };
    assert!(core.apply_fs_changes(&rescan).unwrap());
    assert!(rows(core.tree().unwrap()).contains(&"later.txt@1".to_string()));
}

#[test]
fn entries_inside_an_ignored_folder_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
    std::fs::write(dir.path().join("target/debug/app.exe"), "").unwrap();
    std::fs::write(
        dir.path().join(".gitignore"),
        "target/
",
    )
    .unwrap();
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();
    let root = core.tree().unwrap().root().to_path_buf();
    let tree = core.tree_mut().unwrap();
    // One row per folder here; compact chains have their own test.
    tree.set_compact(false);
    tree.expand(&root.join("target")).unwrap();
    tree.expand(&root.join("target/debug")).unwrap();
    let ignored: Vec<(String, bool)> = tree
        .visible_rows()
        .into_iter()
        .map(|row| (row.entry.name, row.entry.ignored))
        .collect();
    assert_eq!(
        ignored,
        [
            ("target".to_string(), true),
            ("debug".to_string(), true),
            ("app.exe".to_string(), true),
            (".gitignore".to_string(), false),
        ]
    );
}

#[test]
fn close_drops_tree_and_open_file_sets_active_buffer() {
    let (_dir, mut core) = sample();
    assert!(core.active_buffer().is_none());
    core.open_file("root.txt").unwrap();
    let name = core
        .active_buffer()
        .unwrap()
        .path()
        .unwrap()
        .file_name()
        .unwrap();
    assert_eq!(name, "root.txt");
    core.close_workspace();
    assert!(core.tree().is_none() && core.fs_changes().is_none());
}

#[test]
fn watcher_reports_the_parent_of_a_new_file() {
    let (_dir, core) = sample();
    let root = core.tree().unwrap().root().to_path_buf();
    let changes = core.fs_changes().expect("watching");
    std::fs::write(root.join("a/created.txt"), "").unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let wanted = root.join("a");
    loop {
        assert!(
            Instant::now() < deadline,
            "no change reported for {wanted:?}"
        );
        match changes.try_recv() {
            Ok(batch) if batch.rescan || batch.dirs.iter().any(|d| same(d, &wanted)) => break,
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Watcher paths keep the root's spelling; compare ignoring case on Windows.
fn same(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}
