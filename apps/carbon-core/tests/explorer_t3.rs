//! Explorer T3 core: compact folders, move / copy / undo, markers, git status.

use std::path::{Path, PathBuf};
use std::process::Command;

use carbon_core::{Core, CoreError, FileOp, GitStatus, Mark, Row, git_status};

fn labels(core: &Core) -> Vec<String> {
    core.tree()
        .unwrap()
        .visible_rows()
        .into_iter()
        .map(|Row { label, depth, .. }| format!("{label}@{depth}"))
        .collect()
}

fn workspace(files: &[&str]) -> (tempfile::TempDir, Core, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    for file in files {
        let path = dir.path().join(file);
        if file.ends_with('/') {
            std::fs::create_dir_all(&path).unwrap();
        } else {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, file.as_bytes()).unwrap();
        }
    }
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();
    let root = core.tree().unwrap().root().to_path_buf();
    (dir, core, root)
}

#[test]
fn single_folder_chains_share_one_row() {
    let (_dir, mut core, root) = workspace(&["src/main/java/App.java", "README.md"]);
    // Opening `src` opens the whole chain, shown as one row.
    core.tree_mut().unwrap().expand(&root.join("src")).unwrap();
    assert_eq!(
        labels(&core),
        ["src/main/java@0", "App.java@1", "README.md@0"]
    );
    let row = &core.tree().unwrap().visible_rows()[0];
    assert_eq!(row.entry.path, root.join("src/main/java"));

    // Closing the row closes its deepest folder; the chain stays one row.
    core.tree_mut()
        .unwrap()
        .toggle(&root.join("src/main/java"))
        .unwrap();
    assert_eq!(labels(&core), ["src/main/java@0", "README.md@0"]);

    core.tree_mut().unwrap().set_compact(false);
    assert_eq!(labels(&core), ["src@0", "main@1", "java@2", "README.md@0"]);
}

#[test]
fn move_entries_follows_buffers_and_refuses_bad_targets() {
    let (_dir, mut core, root) = workspace(&["a.txt", "b.txt", "docs/", "docs/b.txt", "src/"]);
    core.open_file("a.txt").unwrap();

    let moved = core
        .move_entries(&[root.join("a.txt"), root.join("docs")], root.join("src"))
        .unwrap();
    assert_eq!(moved, [root.join("src/a.txt"), root.join("src/docs")]);
    assert!(root.join("src/docs/b.txt").is_file());
    assert_eq!(
        core.active_buffer().unwrap().path(),
        Some(root.join("src/a.txt").as_path())
    );

    // Into itself, a name clash, outside the workspace: refused, nothing moves.
    assert!(matches!(
        core.move_entries(&[root.join("src")], root.join("src/docs")),
        Err(CoreError::IntoItself(_))
    ));
    assert!(matches!(
        core.move_entries(&[root.join("b.txt")], root.join("src/docs")),
        Err(CoreError::AlreadyExists(_))
    ));
    assert!(
        core.move_entries(&[root.join("b.txt")], root.parent().unwrap())
            .is_err()
    );
    assert!(root.join("b.txt").is_file());
}

#[test]
fn copy_entries_picks_free_names_and_copies_folders() {
    let (_dir, mut core, root) = workspace(&["notes.txt", "docs/", "docs/a.md", ".env"]);
    let first = core.copy_entries(&[root.join("notes.txt")], &root).unwrap();
    let second = core.copy_entries(&[root.join("notes.txt")], &root).unwrap();
    assert_eq!(first, [root.join("notes copy.txt")]);
    assert_eq!(second, [root.join("notes copy 2.txt")]);
    assert_eq!(std::fs::read(&first[0]).unwrap(), b"notes.txt");

    let dotfile = core.copy_entries(&[root.join(".env")], &root).unwrap();
    assert_eq!(dotfile, [root.join(".env copy")]);

    let folder = core
        .copy_entries(&[root.join("docs")], root.join("docs"))
        .err();
    assert!(matches!(folder, Some(CoreError::IntoItself(_))));
    let folder = core.copy_entries(&[root.join("docs")], &root).unwrap();
    assert_eq!(folder, [root.join("docs copy")]);
    assert!(root.join("docs copy/a.md").is_file());

    // Files from outside the workspace (dropped from the OS) can be copied in.
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("dropped.txt"), "x").unwrap();
    let dropped = core
        .copy_entries(&[outside.path().join("dropped.txt")], &root)
        .unwrap();
    assert_eq!(dropped, [root.join("dropped.txt")]);
}

#[test]
fn undo_reverses_rename_and_move_newest_first() {
    let (_dir, mut core, root) = workspace(&["a.txt", "src/"]);
    core.rename(root.join("a.txt"), "b.txt").unwrap();
    core.move_entries(&[root.join("b.txt")], root.join("src"))
        .unwrap();
    assert!(root.join("src/b.txt").is_file());

    assert!(matches!(core.undo_file_op().unwrap(), FileOp::Moved(_)));
    assert!(root.join("b.txt").is_file() && !root.join("src/b.txt").exists());
    let op = core.undo_file_op().unwrap();
    assert_eq!(op.label(), "rename 'a.txt' to 'b.txt'");
    assert!(root.join("a.txt").is_file());
    assert!(matches!(core.undo_file_op(), Err(CoreError::NothingToUndo)));
    assert!(!core.can_undo_file_op());
}

#[test]
fn undo_refuses_when_the_old_name_is_taken_again() {
    let (_dir, mut core, root) = workspace(&["a.txt"]);
    core.rename(root.join("a.txt"), "b.txt").unwrap();
    std::fs::write(root.join("a.txt"), "new").unwrap();
    assert!(matches!(core.undo_file_op(), Err(CoreError::Undo { .. })));
    assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), b"new");
    assert!(root.join("b.txt").is_file());
}

/// Undo of create / copy sends them to the real Recycle Bin / Trash:
/// `cargo test -p carbon-core --test explorer_t3 -- --ignored`.
#[test]
#[ignore]
fn undo_of_create_and_copy_moves_them_to_trash() {
    let (_dir, mut core, root) = workspace(&["a.txt"]);
    core.create_file("", "new.txt").unwrap();
    core.copy_entries(&[root.join("a.txt")], &root).unwrap();
    core.undo_file_op().unwrap();
    assert!(!root.join("a copy.txt").exists());
    core.undo_file_op().unwrap();
    assert!(!root.join("new.txt").exists() && root.join("a.txt").is_file());
}

#[test]
fn marks_roll_up_into_folders_and_follow_moves() {
    let (_dir, mut core, root) = workspace(&["src/a.rs", "src/b.rs", "dest/"]);
    core.marks.set_problems(&root.join("src/a.rs"), 2, 1);
    core.marks.set_problems(&root.join("src/b.rs"), 0, 3);
    core.marks.set_agent_edited(&root.join("src/b.rs"), true);
    assert_eq!(
        core.marks.dir(&root.join("src")),
        Mark {
            errors: 2,
            warnings: 4,
            agent_edited: true
        }
    );
    core.move_entries(&[root.join("src")], root.join("dest"))
        .unwrap();
    assert_eq!(core.marks.file(&root.join("dest/src/a.rs")).errors, 2);
    assert!(core.marks.dir(&root.join("src")).is_empty());
    core.marks.set_problems(&root.join("dest/src/a.rs"), 0, 0);
    assert_eq!(core.marks.dir(&root.join("dest")).errors, 0);
}

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .is_ok_and(|out| out.status.success())
}

#[test]
fn git_status_reports_changed_files_and_folders() {
    let (_dir, core, root) = workspace(&["src/lib.rs", "src/keep.rs", "staged.txt", "old.txt"]);
    // The test repository is made with the git CLI; gitoxide only reads it.
    if !git(&root, &["init", "-q"]) {
        eprintln!("git not installed; skipped");
        return;
    }
    assert!(git(&root, &["add", "src", "old.txt"]));
    assert!(git(&root, &["commit", "-q", "-m", "init"]));
    std::fs::write(root.join("src/lib.rs"), "changed").unwrap();
    std::fs::write(root.join("new.txt"), "").unwrap();
    std::fs::remove_file(root.join("old.txt")).unwrap();
    assert!(git(&root, &["add", "staged.txt"]));

    let status = git_status(&root).unwrap().expect("a repository");
    assert_eq!(
        status.file(&root.join("src/lib.rs")),
        Some(GitStatus::Modified)
    );
    assert_eq!(status.file(&root.join("src/keep.rs")), None);
    assert_eq!(
        status.file(&root.join("new.txt")),
        Some(GitStatus::Untracked)
    );
    assert_eq!(
        status.file(&root.join("staged.txt")),
        Some(GitStatus::Added)
    );
    assert_eq!(status.file(&root.join("old.txt")), Some(GitStatus::Deleted));
    assert_eq!(status.dir(&root.join("src")), Some(GitStatus::Modified));
    drop(core);

    let plain = tempfile::tempdir().unwrap();
    // A temp folder may sit inside some repository on odd setups; only check
    // that a folder without one gives no error.
    assert!(git_status(plain.path()).is_ok());
}
