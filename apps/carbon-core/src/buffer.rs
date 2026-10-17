use std::collections::HashMap;
use std::io::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ropey::{LineType, Rope};

use crate::id::define_id;
use crate::{CoreError, Result};

define_id!(
    /// An open text buffer.
    BufferId
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    /// The first line break decides; files without one get the platform default.
    fn detect(text: &str) -> Self {
        match text.find('\n') {
            Some(ix) if ix > 0 && text.as_bytes()[ix - 1] == b'\r' => Self::Crlf,
            Some(_) => Self::Lf,
            None if cfg!(windows) => Self::Crlf,
            None => Self::Lf,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// A text buffer, kept exactly as on disk (line endings are not converted).
/// Offsets are byte offsets into the UTF-8 text and must fall on char boundaries.
pub struct Buffer {
    id: BufferId,
    path: Option<PathBuf>,
    rope: Rope,
    line_ending: LineEnding,
    revision: u64,
    saved_revision: u64,
    /// Modified time of the file when last loaded or saved; a different time
    /// on disk means someone else changed it.
    disk_mtime: Option<SystemTime>,
    /// The file changed on disk while this buffer had unsaved changes; the
    /// user picks reload or keep.
    disk_conflict: bool,
}

impl Buffer {
    fn new(id: BufferId, path: Option<PathBuf>, text: &str) -> Self {
        Self {
            id,
            path,
            rope: Rope::from_str(text),
            line_ending: LineEnding::detect(text),
            revision: 0,
            saved_revision: 0,
            disk_mtime: None,
            disk_conflict: false,
        }
    }

    fn load(id: BufferId, path: PathBuf) -> Result<Self> {
        let text = read_text(&path)?;
        let mut buffer = Self::new(id, Some(path), &text);
        buffer.disk_mtime = buffer.path().and_then(mtime);
        Ok(buffer)
    }

    pub fn id(&self) -> BufferId {
        self.id
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len() == 0
    }

    /// Number of lines (an empty buffer has one).
    pub fn line_count(&self) -> usize {
        self.rope.len_lines(LineType::LF_CR)
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Bumped on every edit; lets views tell whether they are stale.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    /// Takes the whole text from an editor view that edited its own copy
    /// (same rope type, so this is a cheap clone on the caller's side).
    pub fn set_rope(&mut self, rope: Rope) {
        self.rope = rope;
        self.revision += 1;
    }

    /// Whether the file on disk was changed by someone else since it was
    /// loaded or saved. False for a buffer with no file, or a deleted file.
    pub fn changed_on_disk(&self) -> bool {
        match (self.path().and_then(mtime), self.disk_mtime) {
            (Some(now), Some(known)) => now != known,
            (Some(_), None) => self.path.is_some(),
            (None, _) => false,
        }
    }

    /// The user keeps their unsaved text over a changed file: stop reporting
    /// this change (the next save overwrites it).
    pub fn ignore_disk_change(&mut self) {
        self.disk_mtime = self.path().and_then(mtime);
        self.disk_conflict = false;
    }

    pub fn has_disk_conflict(&self) -> bool {
        self.disk_conflict
    }

    /// Flags a conflict; false when it was already flagged.
    pub(crate) fn flag_disk_conflict(&mut self) -> bool {
        !std::mem::replace(&mut self.disk_conflict, true)
    }

    /// Re-reads the file, dropping unsaved changes.
    pub fn reload(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or(CoreError::NoPath)?;
        let text = read_text(&path)?;
        self.rope = Rope::from_str(&text);
        self.line_ending = LineEnding::detect(&text);
        self.revision += 1;
        self.saved_revision = self.revision;
        self.disk_mtime = mtime(&path);
        self.disk_conflict = false;
        Ok(())
    }

    pub fn insert(&mut self, offset: usize, text: &str) -> Result<()> {
        self.rope
            .try_insert(offset, text)
            .map_err(|_| CoreError::InvalidOffset(offset))?;
        self.revision += 1;
        Ok(())
    }

    pub fn remove(&mut self, range: Range<usize>) -> Result<()> {
        let start = range.start;
        self.rope
            .try_remove(range)
            .map_err(|_| CoreError::InvalidOffset(start))?;
        self.revision += 1;
        Ok(())
    }

    /// Replaces `range` with `text` as one edit (one revision).
    pub fn replace(&mut self, range: Range<usize>, text: &str) -> Result<()> {
        let start = range.start;
        // Validate first so a failed replace changes nothing.
        if range.start > range.end
            || range.end > self.rope.len()
            || !self.rope.is_char_boundary(range.start)
            || !self.rope.is_char_boundary(range.end)
        {
            return Err(CoreError::InvalidOffset(start));
        }
        self.rope.remove(range);
        self.rope.insert(start, text);
        self.revision += 1;
        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or(CoreError::NoPath)?;
        self.write_to(&path)
    }

    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        self.write_to(&path)?;
        self.path = Some(path);
        Ok(())
    }

    /// Atomic: writes a temp file beside the target, then renames it over, so a
    /// crash never leaves a half-written file.
    fn write_to(&mut self, path: &Path) -> Result<()> {
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut temp =
            tempfile::NamedTempFile::new_in(dir).map_err(|err| CoreError::io(dir, err))?;
        for chunk in self.rope.chunks() {
            temp.write_all(chunk.as_bytes())
                .map_err(|err| CoreError::io(temp.path(), err))?;
        }
        temp.as_file()
            .sync_all()
            .map_err(|err| CoreError::io(temp.path(), err))?;
        temp.persist(path)
            .map_err(|err| CoreError::io(path, err.error))?;
        self.saved_revision = self.revision;
        self.disk_mtime = mtime(path);
        self.disk_conflict = false;
        Ok(())
    }
}

fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|err| CoreError::io(path, err))?;
    String::from_utf8(bytes).map_err(|_| CoreError::NotUtf8(path.to_path_buf()))
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// All open buffers. A file is open at most once.
#[derive(Default)]
pub struct BufferStore {
    next_id: u64,
    buffers: HashMap<BufferId, Buffer>,
}

impl BufferStore {
    fn next_id(&mut self) -> BufferId {
        self.next_id += 1;
        BufferId(self.next_id)
    }

    /// Opens `path` (absolute) or returns the buffer that already has it open.
    pub fn open(&mut self, path: impl Into<PathBuf>) -> Result<BufferId> {
        let path = path.into();
        let path = dunce::canonicalize(&path).map_err(|err| CoreError::io(&path, err))?;
        if let Some(buffer) = self
            .buffers
            .values()
            .find(|b| b.path() == Some(path.as_path()))
        {
            return Ok(buffer.id());
        }
        let id = self.next_id();
        self.buffers.insert(id, Buffer::load(id, path)?);
        Ok(id)
    }

    /// The buffer that has `path` open, if any (`path` as stored: canonical).
    pub fn find(&self, path: &Path) -> Option<BufferId> {
        self.buffers
            .values()
            .find(|b| b.path() == Some(path))
            .map(Buffer::id)
    }

    /// A new buffer with no file; save it with `save_as`.
    pub fn create_untitled(&mut self) -> BufferId {
        let id = self.next_id();
        self.buffers.insert(id, Buffer::new(id, None, ""));
        id
    }

    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.get(&id)
    }

    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.buffers.get_mut(&id)
    }

    /// After `old` was renamed to `new`: buffers of it, or of files under it,
    /// follow the move.
    pub fn moved(&mut self, old: &Path, new: &Path) {
        for buffer in self.buffers.values_mut() {
            if let Some(rest) = buffer.path().and_then(|path| path.strip_prefix(old).ok()) {
                buffer.path = Some(if rest.as_os_str().is_empty() {
                    new.to_path_buf()
                } else {
                    new.join(rest)
                });
            }
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = BufferId> + '_ {
        self.buffers.keys().copied()
    }

    /// Closes a buffer. Refuses one with unsaved changes unless `force`.
    pub fn close(&mut self, id: BufferId, force: bool) -> Result<()> {
        let buffer = self.buffers.get(&id).ok_or(CoreError::NotFound("buffer"))?;
        if buffer.is_dirty() && !force {
            return Err(CoreError::Dirty);
        }
        self.buffers.remove(&id);
        Ok(())
    }
}
