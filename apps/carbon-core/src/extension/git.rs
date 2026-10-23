//! Extension packages from git (gix, not the user's git; from the shelved
//! workflow branch). A repository URL (https, ssh, a
//! local path) and a revision: a tag, a branch, or a full commit id (pinned).
//! Only that one commit is fetched (shallow, into a temporary bare
//! repository), and its tree is written out to a temporary folder with the
//! package rules checked first: no links, no submodules, Q22 paths, the file
//! and size limits. The folder then goes through `read_package` like any
//! other. Nothing from the repository is run (no hooks, no filters).
//! [`FetchControl`] cancels a fetch and reports its progress.

use std::fmt;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gix::progress::tree;
use gix::remote::fetch::Shallow;

use super::{
    InstallSource, MAX_PACKAGE_BYTES, MAX_PACKAGE_FILES, Package, check_package_path, read_package,
};

/// A commit written out to a temporary folder; the folder lives as long as
/// any clone of this.
#[derive(Clone, Debug)]
pub struct GitCheckout {
    pub url: String,
    /// What was asked for: a tag, a branch, `HEAD` or the commit id itself.
    pub rev: String,
    /// The full commit id fetched.
    pub commit: String,
    /// The package's folder inside the repository (`extensions/rust`);
    /// `None` when it is the repository root.
    pub path: Option<String>,
    /// The package root (inside the temporary folder).
    pub root: PathBuf,
    _temp: Arc<tempfile::TempDir>,
}

impl PartialEq for GitCheckout {
    fn eq(&self, other: &Self) -> bool {
        (&self.url, &self.rev, &self.commit, &self.path, &self.root)
            == (
                &other.url,
                &other.rev,
                &other.commit,
                &other.path,
                &other.root,
            )
    }
}

impl Eq for GitCheckout {}

impl fmt::Display for GitCheckout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let short = &self.commit[..self.commit.len().min(10)];
        let path = self
            .path
            .as_deref()
            .map(|path| format!(" /{path}"))
            .unwrap_or_default();
        if self.rev == self.commit {
            write!(f, "{}{path} @ {short}", self.url)
        } else {
            write!(f, "{}{path} @ {} ({short})", self.url, self.rev)
        }
    }
}

/// Cancels a running fetch and tells how far it got. Shared between the
/// thread that fetches and the UI.
pub struct FetchControl {
    interrupt: AtomicBool,
    progress: Arc<tree::Root>,
}

impl Default for FetchControl {
    fn default() -> Self {
        Self {
            interrupt: AtomicBool::new(false),
            progress: tree::Root::new(),
        }
    }
}

impl FetchControl {
    /// Asks the fetch to stop; gix checks between steps, so it may take a
    /// while (seconds, more on a big pack).
    pub fn cancel(&self) {
        self.interrupt.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.interrupt.load(Ordering::Relaxed)
    }

    /// The step that is moving, e.g. `read pack: 12.3 MB`; `None` before
    /// the first one starts.
    pub fn status(&self) -> Option<String> {
        let mut tasks = Vec::new();
        self.progress.sorted_snapshot(&mut tasks);
        tasks.iter().rev().find_map(|(_, task)| {
            let value = task.progress.as_ref()?;
            let step = value.step.load(Ordering::Relaxed);
            let unit = value.unit.as_ref()?;
            (step > 0)
                .then(|| format!("{}: {}", task.name, unit.display(step, value.done_at, None)))
        })
    }
}

/// A full hexadecimal commit id (40 characters, SHA-1).
pub fn is_commit_id(rev: &str) -> bool {
    rev.len() == 40 && rev.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Fetches `rev` of `url` (empty: the default branch) and reads it as a
/// package. A commit id must be what the server sends back. Blocking and
/// network-bound: run it off the UI thread; `control` cancels it.
pub fn fetch_git(url: &str, rev: &str, control: &FetchControl) -> Result<Package, Vec<String>> {
    fetch_git_path(url, rev, None, control)
}

/// Like [`fetch_git`], for a package in folder `path` of the repository
/// (Carbon's official extensions live in `extensions/<name>` of its repo).
pub fn fetch_git_path(
    url: &str,
    rev: &str,
    path: Option<&str>,
    control: &FetchControl,
) -> Result<Package, Vec<String>> {
    let rev = rev.trim();
    let path = path.map(str::trim).filter(|path| !path.is_empty());
    if let Some(path) = path {
        check_package_path(path).map_err(|err| vec![format!("folder {err}")])?;
    }
    let (temp, repo) = fetch(url, rev, control).map_err(|err| vec![err])?;
    let commit = repo
        .head_commit()
        .map_err(|err| vec![format!("{url}: no commit for `{rev}`: {}", chain(&err))])?;
    let commit_id = commit.id.to_string();
    if is_commit_id(rev) && !commit_id.eq_ignore_ascii_case(rev) {
        return Err(vec![format!(
            "{url}: asked for commit {rev} but got {commit_id}"
        )]);
    }
    let root = temp.path().join("package");
    let fail = |err: &dyn std::error::Error| vec![format!("{url}: {}", chain(err))];
    let mut tree = commit.tree().map_err(|err| fail(&err))?;
    if let Some(path) = path {
        let entry = tree
            .lookup_entry_by_path(path)
            .map_err(|err| fail(&err))?
            .filter(|entry| entry.mode().is_tree())
            .ok_or_else(|| vec![format!("{url}: no folder `{path}` at {}", commit.id)])?;
        tree = repo
            .find_tree(entry.object_id())
            .map_err(|err| fail(&err))?;
    }
    write_tree(&repo, &tree, &root)?;
    read_package(InstallSource::Git(GitCheckout {
        url: url.to_string(),
        rev: if rev.is_empty() {
            "HEAD".into()
        } else {
            rev.to_string()
        },
        commit: commit_id,
        path: path.map(str::to_string),
        root,
        _temp: Arc::new(temp),
    }))
}

/// Files at the root of `url`'s default branch (the catalog's
/// `catalog.json` and its signature), each at most `limit` bytes; `None` for
/// one that is not there.
pub(super) fn fetch_files(
    url: &str,
    paths: &[&str],
    limit: u64,
    control: &FetchControl,
) -> Result<Vec<Option<Vec<u8>>>, String> {
    let (_temp, repo) = fetch(url, "", control)?;
    let commit = repo
        .head_commit()
        .map_err(|err| format!("{url}: no commit: {}", chain(&err)))?;
    let fail = |err: &dyn std::error::Error| format!("{url}: {}", chain(err));
    let tree = commit.tree().map_err(|err| fail(&err))?;
    let mut files = Vec::new();
    for path in paths {
        let Some(entry) = tree
            .lookup_entry_by_path(path)
            .map_err(|err| fail(&err))?
            .filter(|entry| entry.mode().is_blob())
        else {
            files.push(None);
            continue;
        };
        let header = repo
            .find_header(entry.object_id())
            .map_err(|err| fail(&err))?;
        if header.size() > limit {
            return Err(format!("{url}: {path} is larger than {} KB", limit / 1024));
        }
        let blob = repo
            .find_blob(entry.object_id())
            .map_err(|err| fail(&err))?;
        files.push(Some(blob.data.clone()));
    }
    Ok(files)
}

/// A shallow bare clone of `url` holding just `rev`'s commit, in a new
/// temporary folder; `HEAD` points at it.
fn fetch(
    url: &str,
    rev: &str,
    control: &FetchControl,
) -> Result<(tempfile::TempDir, gix::Repository), String> {
    let fail = |err: &dyn std::error::Error| format!("{url}: {}", chain(err));
    if url.trim().is_empty() {
        return Err("no repository URL".into());
    }
    let temp = tempfile::Builder::new()
        .prefix("carbon-extension-")
        .tempdir()
        .map_err(|err| fail(&err))?;
    let mut prepare = gix::prepare_clone_bare(url, temp.path().join("repo"))
        .map_err(|err| fail(&err))?
        .with_shallow(Shallow::DepthAtRemote(NonZeroU32::MIN));
    if is_commit_id(rev) {
        prepare = prepare
            .with_revision(Some(rev.to_ascii_lowercase()))
            .map_err(|err| fail(&err))?;
    } else if !rev.is_empty() {
        prepare = prepare.with_ref_name(Some(rev)).map_err(|err| fail(&err))?;
    }
    let (repo, _) = prepare
        .fetch_only(control.progress.add_child(url), &control.interrupt)
        .map_err(|err| fail(&err))?;
    Ok((temp, repo))
}

/// Writes `tree` to `root`, refusing anything a package may not hold.
fn write_tree(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    root: &Path,
) -> Result<(), Vec<String>> {
    let fail = |err: &dyn std::error::Error| vec![chain(err)];
    let mut recorder = gix::traverse::tree::Recorder::default();
    tree.traverse()
        .breadthfirst(&mut recorder)
        .map_err(|err| fail(&err))?;
    let mut problems = Vec::new();
    let mut blobs = Vec::new();
    let mut bytes = 0u64;
    for entry in &recorder.records {
        let path = entry.filepath.to_string();
        if entry.mode.is_tree() {
            continue;
        }
        if entry.mode.is_link() {
            problems.push(format!("`{path}`: links are not allowed in a package"));
        } else if entry.mode.is_commit() {
            problems.push(format!("`{path}`: submodules are not allowed in a package"));
        } else if let Err(problem) = check_package_path(&path) {
            problems.push(problem);
        } else {
            let header = repo.find_header(entry.oid).map_err(|err| fail(&err))?;
            bytes += header.size();
            blobs.push((path, entry.oid));
        }
    }
    if blobs.len() > MAX_PACKAGE_FILES {
        problems.push(format!("more than {MAX_PACKAGE_FILES} files"));
    }
    if bytes > MAX_PACKAGE_BYTES {
        problems.push(format!(
            "larger than {} MB",
            MAX_PACKAGE_BYTES / 1024 / 1024
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    for (path, oid) in blobs {
        let blob = repo.find_blob(oid).map_err(|err| fail(&err))?;
        let target = root.join(&path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| vec![format!("{}: {err}", parent.display())])?;
        }
        std::fs::write(&target, &blob.data)
            .map_err(|err| vec![format!("{}: {err}", target.display())])?;
    }
    Ok(())
}

/// An error and its causes, `: `-separated: gix's own message is often just
/// "Transport handshake failed" with the reason (DNS, TLS, 404) below it.
fn chain(err: &dyn std::error::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let cause_text = cause.to_string();
        if !text.contains(&cause_text) {
            text.push_str(": ");
            text.push_str(&cause_text);
        }
        source = cause.source();
    }
    text
}
