// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::path::PathBuf;

use git_core::{ChangeSet, GixBackend};
use support::TestRepo;

fn classify(repo: &TestRepo, paths: &[&str]) -> ChangeSet {
    let backend = GixBackend::discover(repo.path()).unwrap();
    let root = backend_root(&backend);
    let paths: Vec<PathBuf> = paths.iter().map(|p| root.join(p)).collect();
    backend.classify_changes(&paths)
}

fn backend_root(backend: &GixBackend) -> PathBuf {
    use git_core::GitBackend;
    backend.path().to_path_buf()
}

fn repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.commit(".gitignore", "target/\n*.log\n", "Ignore");
    repo
}

#[test]
fn build_output_in_ignored_folders_is_dropped() {
    let repo = repo();
    assert!(classify(&repo, &["target/debug/deps/x.o", "target/release/gitloom"]).is_empty());
    assert!(classify(&repo, &["logs/build.log"]).is_empty());
}

#[test]
fn tracked_and_untracked_files_need_a_status_refresh() {
    let repo = repo();
    let change = classify(&repo, &["target/debug/x", "src/main.rs"]);
    assert_eq!(
        change,
        ChangeSet {
            status: true,
            refs: false
        }
    );
}

#[test]
fn git_internals() {
    let repo = repo();
    assert_eq!(
        classify(&repo, &[".git/index"]),
        ChangeSet {
            status: true,
            refs: false
        }
    );
    assert_eq!(
        classify(&repo, &[".git/refs/heads/main", ".git/objects/ab/cd"]),
        ChangeSet {
            status: false,
            refs: true
        }
    );
    assert!(classify(
        &repo,
        &[".git/objects/ab/cd", ".git/index.lock", ".git/logs/HEAD"]
    )
    .is_empty());
}

#[test]
fn paths_outside_the_repository_are_dropped() {
    let repo = repo();
    let backend = GixBackend::discover(repo.path()).unwrap();
    let change = backend.classify_changes(&[PathBuf::from("/somewhere/else/file.txt")]);
    assert!(change.is_empty());
}
