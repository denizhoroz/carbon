//! File system watcher for the explorer: reports which folders' listings may
//! have changed, in debounced batches.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::event::{EventKind, ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::{CoreError, Result};

/// Quiet time that ends a batch: a `git checkout` or build touching many files
/// arrives as one batch instead of hundreds.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// One debounced batch of changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsChanges {
    /// Folders whose listing may differ (parents of created, removed or renamed entries).
    pub dirs: Vec<PathBuf>,
    /// Entries that were created, changed, removed or renamed (outside `.git`);
    /// open buffers of them may need a reload.
    pub files: Vec<PathBuf>,
    /// Events were lost (the OS buffer overflowed); every loaded folder needs a refresh.
    pub rescan: bool,
    /// Something changed that git status may see: file content, or `.git`
    /// itself (index, refs, `HEAD`).
    pub git: bool,
}

/// Watches a folder recursively until dropped.
pub struct FsWatcher {
    _watcher: RecommendedWatcher,
    changes: async_channel::Receiver<FsChanges>,
}

impl FsWatcher {
    pub fn new(root: &Path) -> Result<Self> {
        let (raw_sender, raw) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(move |event| {
            _ = raw_sender.send(event);
        })
        .map_err(|err| watch_error(root, err))?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|err| watch_error(root, err))?;

        let (sender, changes) = async_channel::unbounded();
        // Ends when the watcher (and with it `raw_sender`) is dropped.
        std::thread::Builder::new()
            .name("carbon-fs-watch".into())
            .spawn(move || debounce(raw, sender))
            .map_err(|err| CoreError::io(root, err))?;
        Ok(Self {
            _watcher: watcher,
            changes,
        })
    }

    /// Debounced batches; clone it into a task that applies them.
    pub fn changes(&self) -> async_channel::Receiver<FsChanges> {
        self.changes.clone()
    }
}

fn debounce(
    raw: mpsc::Receiver<notify::Result<notify::Event>>,
    sender: async_channel::Sender<FsChanges>,
) {
    // Blocks until the first event of a batch, then collects until it goes quiet.
    while let Ok(first) = raw.recv() {
        let mut batch = Batch::default();
        batch.collect(first);
        loop {
            match raw.recv_timeout(DEBOUNCE) {
                Ok(event) => batch.collect(event),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        if batch.dirs.is_empty() && batch.files.is_empty() && !batch.rescan && !batch.git {
            continue;
        }
        let batch = FsChanges {
            dirs: batch.dirs.into_iter().collect(),
            files: batch.files.into_iter().collect(),
            rescan: batch.rescan,
            git: batch.git,
        };
        if sender.send_blocking(batch).is_err() {
            return;
        }
    }
}

#[derive(Default)]
struct Batch {
    dirs: HashSet<PathBuf>,
    files: HashSet<PathBuf>,
    rescan: bool,
    git: bool,
}

impl Batch {
    fn collect(&mut self, event: notify::Result<notify::Event>) {
        let event = match event {
            Ok(event) => event,
            Err(_) => {
                self.rescan = true;
                self.git = true;
                return;
            }
        };
        if event.need_rescan() {
            self.rescan = true;
            self.git = true;
            return;
        }
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        // Content edits do not change a listing; Windows reports them as `Modify(Any)`.
        let listing_changed = match event.kind {
            EventKind::Create(_) | EventKind::Remove(_) => true,
            EventKind::Modify(ModifyKind::Name(_)) => true,
            EventKind::Any | EventKind::Other => true,
            EventKind::Modify(_) | EventKind::Access(_) => false,
        };
        for path in event.paths {
            let mut parts = path.components().map(|part| part.as_os_str());
            if parts.any(|part| part == ".git") {
                // Object writes alone change nothing git status shows.
                if parts.next().is_some_and(|next| next != "objects") {
                    self.git = true;
                }
                continue;
            }
            self.git = true;
            if listing_changed && let Some(parent) = path.parent() {
                self.dirs.insert(parent.to_path_buf());
            }
            self.files.insert(path);
        }
    }
}

fn watch_error(root: &Path, err: notify::Error) -> CoreError {
    CoreError::Watch(format!("{}: {err}", root.display()))
}
