use carbon_core::{Branch, Core, JobStatus, JobStore, current_branch};

fn repo_with_head(head: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join(".git/HEAD"), head).unwrap();
    dir
}

#[test]
fn reads_branch_from_head() {
    let dir = repo_with_head("ref: refs/heads/feature/status-bar\n");
    assert_eq!(
        current_branch(dir.path()),
        Some(Branch::Named("feature/status-bar".into()))
    );
}

#[test]
fn detached_head_shows_short_commit() {
    let dir = repo_with_head("3f9c2a1d8e7b6c5a4f3e2d1c0b9a8f7e6d5c4b3a\n");
    let branch = current_branch(dir.path()).unwrap();
    assert_eq!(branch, Branch::Detached("3f9c2a1".into()));
    assert_eq!(branch.label(), "3f9c2a1");
}

#[test]
fn finds_repo_from_subfolder_and_worktree_file() {
    let dir = repo_with_head("ref: refs/heads/main\n");
    std::fs::create_dir_all(dir.path().join("src/deep")).unwrap();
    assert_eq!(
        current_branch(&dir.path().join("src/deep")),
        Some(Branch::Named("main".into()))
    );

    // Worktree: `.git` is a file pointing at the real git dir.
    let worktree = tempfile::tempdir().unwrap();
    let git_dir = dir.path().join(".git/worktrees/wt");
    std::fs::create_dir_all(&git_dir).unwrap();
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/wt-branch\n").unwrap();
    std::fs::write(worktree.path().join(".git"), format!("gitdir: {}\n", git_dir.display())).unwrap();
    assert_eq!(
        current_branch(worktree.path()),
        Some(Branch::Named("wt-branch".into()))
    );
}

#[test]
fn core_branch_follows_workspace() {
    let mut core = Core::new();
    assert_eq!(core.branch(), None);
    let dir = repo_with_head("ref: refs/heads/main\n");
    core.open_workspace(dir.path()).unwrap();
    assert_eq!(core.branch(), Some(Branch::Named("main".into())));
}

#[test]
fn jobs_track_running_and_finished() {
    let mut jobs = JobStore::default();
    let index = jobs.start("Indexing");
    let build = jobs.start("cargo build");
    assert_eq!(jobs.running().count(), 2);

    jobs.finish(index, Ok(()));
    jobs.finish(build, Err("exit code 101".into()));
    assert_eq!(jobs.running().count(), 0);
    assert_eq!(jobs.get(index).unwrap().status(), &JobStatus::Succeeded);
    assert_eq!(
        jobs.get(build).unwrap().status(),
        &JobStatus::Failed("exit code 101".into())
    );

    jobs.clear_finished();
    assert_eq!(jobs.list().count(), 0);
}
