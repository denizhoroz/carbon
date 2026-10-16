use carbon_core::{BufferStore, Core, CoreError, LineEnding};

#[test]
fn edit_mark_dirty_save_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "hello\nworld\n").unwrap();

    let mut store = BufferStore::default();
    let id = store.open(&path).unwrap();
    let buffer = store.get_mut(id).unwrap();
    assert_eq!(buffer.line_count(), 3);
    assert!(!buffer.is_dirty());

    buffer.insert(5, ",").unwrap();
    buffer.replace(7..12, "carbon").unwrap();
    assert_eq!(buffer.text(), "hello,\ncarbon\n");
    assert!(buffer.is_dirty());
    assert_eq!(buffer.revision(), 2);

    buffer.save().unwrap();
    assert!(!buffer.is_dirty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello,\ncarbon\n");
}

#[test]
fn keeps_crlf_and_reports_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("win.txt");
    std::fs::write(&path, "a\r\nb\r\n").unwrap();

    let mut store = BufferStore::default();
    let id = store.open(&path).unwrap();
    let buffer = store.get_mut(id).unwrap();
    assert_eq!(buffer.line_ending(), LineEnding::Crlf);
    assert_eq!(buffer.line_count(), 3);
    buffer.insert(buffer.len(), "c\r\n").unwrap();
    buffer.save().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"a\r\nb\r\nc\r\n");
}

#[test]
fn rejects_bad_offsets_without_changing_text() {
    let mut store = BufferStore::default();
    let id = store.create_untitled();
    let buffer = store.get_mut(id).unwrap();
    buffer.insert(0, "héllo").unwrap();
    let revision = buffer.revision();

    // Byte 2 is inside 'é' (2 bytes); 99 is past the end.
    assert!(matches!(
        buffer.insert(2, "x"),
        Err(CoreError::InvalidOffset(2))
    ));
    assert!(matches!(
        buffer.remove(0..99),
        Err(CoreError::InvalidOffset(0))
    ));
    assert!(matches!(
        buffer.replace(1..2, "x"),
        Err(CoreError::InvalidOffset(1))
    ));
    assert_eq!(buffer.text(), "héllo");
    assert_eq!(buffer.revision(), revision);
}

#[test]
fn refuses_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.bin");
    std::fs::write(&path, [0xff, 0xfe, 0x00, 0x80]).unwrap();
    let mut store = BufferStore::default();
    assert!(matches!(store.open(&path), Err(CoreError::NotUtf8(_))));
}

#[test]
fn opens_each_file_once() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "x").unwrap();
    let mut core = Core::new();
    core.open_workspace(dir.path()).unwrap();

    let first = core.open_file("a.txt").unwrap();
    let again = core.open_file("./a.txt").unwrap();
    let absolute = core
        .open_file(core.workspace().unwrap().root().join("a.txt"))
        .unwrap();
    assert_eq!(first, again);
    assert_eq!(first, absolute);
    assert_eq!(core.buffers.ids().count(), 1);
}

#[test]
fn close_protects_unsaved_changes() {
    let mut store = BufferStore::default();
    let id = store.create_untitled();
    store.get_mut(id).unwrap().insert(0, "draft").unwrap();

    assert!(matches!(store.close(id, false), Err(CoreError::Dirty)));
    store.close(id, true).unwrap();
    assert!(store.get(id).is_none());
    assert!(matches!(
        store.close(id, true),
        Err(CoreError::NotFound("buffer"))
    ));
}

#[test]
fn untitled_needs_save_as() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = BufferStore::default();
    let id = store.create_untitled();
    let buffer = store.get_mut(id).unwrap();
    buffer.insert(0, "new").unwrap();

    assert!(matches!(buffer.save(), Err(CoreError::NoPath)));
    let path = dir.path().join("new.txt");
    buffer.save_as(&path).unwrap();
    assert_eq!(buffer.path(), Some(path.as_path()));
    assert!(!buffer.is_dirty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}
