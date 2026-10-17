//! Editor tier 1: buffers taking text from the editor view, saving, and
//! following changes made on disk by other programs.

use std::path::Path;
use std::time::{Duration, SystemTime};

use carbon_core::{Core, CoreError, FsChanges, Rope};

fn workspace(files: &[(&str, &str)]) -> (tempfile::TempDir, Core) {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).unwrap();
    }
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();
    (dir, core)
}

/// Writes `text` as another program would, with a modified time that
/// differs from any earlier write (file times can be coarse).
fn write_elsewhere(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(60))
        .unwrap();
}

fn changed(path: &Path) -> FsChanges {
    FsChanges {
        files: vec![path.to_path_buf()],
        ..Default::default()
    }
}

#[test]
fn set_rope_marks_dirty_and_save_clears_it() {
    let (dir, mut core) = workspace(&[("a.txt", "one\n")]);
    let id = core.open_file("a.txt").unwrap();
    let buffer = core.buffers.get_mut(id).unwrap();
    buffer.set_rope(Rope::from_str("one\ntwo\n"));
    assert!(buffer.is_dirty());

    core.save_buffer(id).unwrap();
    let buffer = core.buffers.get(id).unwrap();
    assert!(!buffer.is_dirty());
    assert!(
        !buffer.changed_on_disk(),
        "our own save is not a disk change"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "one\ntwo\n"
    );
}

#[test]
fn clean_buffer_reloads_when_the_file_changes() {
    let (dir, mut core) = workspace(&[("a.txt", "old\n")]);
    let id = core.open_file("a.txt").unwrap();
    let path = dir.path().join("a.txt");
    write_elsewhere(&path, "new\n");

    let sync = core.sync_buffers_with_disk(&changed(&path));
    assert_eq!(sync.reloaded, [id]);
    assert!(sync.conflicts.is_empty());
    let buffer = core.buffers.get(id).unwrap();
    assert_eq!(buffer.text(), "new\n");
    assert!(!buffer.is_dirty());

    // Nothing more to do for the same change.
    assert!(
        core.sync_buffers_with_disk(&changed(&path))
            .reloaded
            .is_empty()
    );
}

#[test]
fn dirty_buffer_reports_a_conflict_until_ignored() {
    let (dir, mut core) = workspace(&[("a.txt", "old\n")]);
    let id = core.open_file("a.txt").unwrap();
    core.buffers
        .get_mut(id)
        .unwrap()
        .set_rope(Rope::from_str("mine\n"));
    let path = dir.path().join("a.txt");
    write_elsewhere(&path, "theirs\n");

    let sync = core.sync_buffers_with_disk(&changed(&path));
    assert_eq!(sync.conflicts, [id]);
    assert_eq!(core.buffers.get(id).unwrap().text(), "mine\n");
    assert!(core.buffers.get(id).unwrap().has_disk_conflict());
    // Already flagged: not reported again.
    assert!(
        core.sync_buffers_with_disk(&changed(&path))
            .conflicts
            .is_empty()
    );

    core.buffers.get_mut(id).unwrap().ignore_disk_change();
    assert!(!core.buffers.get(id).unwrap().has_disk_conflict());
    assert!(
        core.sync_buffers_with_disk(&changed(&path))
            .conflicts
            .is_empty()
    );

    // Reload drops the unsaved text.
    core.buffers.get_mut(id).unwrap().reload().unwrap();
    assert_eq!(core.buffers.get(id).unwrap().text(), "theirs\n");
}

#[test]
fn rescan_checks_every_buffer() {
    let (dir, mut core) = workspace(&[("a.txt", "old\n")]);
    let id = core.open_file("a.txt").unwrap();
    write_elsewhere(&dir.path().join("a.txt"), "new\n");
    let rescan = FsChanges {
        rescan: true,
        ..Default::default()
    };
    assert_eq!(core.sync_buffers_with_disk(&rescan).reloaded, [id]);
}

#[test]
fn close_buffer_refuses_unsaved_and_clears_active() {
    let (_dir, mut core) = workspace(&[("a.txt", "x")]);
    let id = core.open_file("a.txt").unwrap();
    core.buffers
        .get_mut(id)
        .unwrap()
        .set_rope(Rope::from_str("y"));
    assert!(matches!(
        core.close_buffer(id, false),
        Err(CoreError::Dirty)
    ));
    core.close_buffer(id, true).unwrap();
    assert!(core.active_buffer().is_none());
    assert!(core.buffers.get(id).is_none());
}

#[test]
fn untitled_buffer_saves_as_a_file() {
    let (dir, mut core) = workspace(&[]);
    let first = core.new_file();
    let second = core.new_file();
    assert_eq!(core.active_buffer().unwrap().id(), second);
    assert_eq!(core.buffers.get(first).unwrap().name(), "Untitled-1");
    assert_eq!(core.buffers.get(second).unwrap().name(), "Untitled-2");
    assert!(core.dirty_buffers().is_empty());

    core.buffers
        .get_mut(first)
        .unwrap()
        .set_rope(Rope::from_str("hello\n"));
    assert_eq!(core.dirty_buffers(), [first]);
    assert!(matches!(core.save_buffer(first), Err(CoreError::NoPath)));

    let path = dir.path().join("hello.txt");
    core.save_buffer_as(first, &path).unwrap();
    let buffer = core.buffers.get(first).unwrap();
    assert_eq!(buffer.name(), "hello.txt");
    assert_eq!(
        buffer.path(),
        Some(dunce::canonicalize(&path).unwrap().as_path())
    );
    assert!(core.dirty_buffers().is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello\n");
    // Opening the file now finds the same buffer.
    assert_eq!(core.open_file("hello.txt").unwrap(), first);
}
