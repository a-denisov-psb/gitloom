// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{CliBackend, Error, GitBackend, GitWriter, GixBackend};
use support::TestRepo;

fn isolated() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.commit("a.txt", "a\n", "Base");
    repo
}

fn backend(repo: &TestRepo) -> GixBackend {
    GixBackend::discover(repo.path()).unwrap()
}

#[test]
fn push_includes_untracked_files() {
    let repo = isolated();
    let base = repo.head();
    repo.write("a.txt", "changed\n");
    repo.write("new.txt", "untracked\n");
    CliBackend::new(repo.path()).stash_push(None).unwrap();

    let tree = backend(&repo).working_tree().unwrap();
    assert!(
        tree.is_empty(),
        "{tree:?} / git: {}",
        repo.git(&["status", "--porcelain"])
    );
    let stashes = backend(&repo).stashes().unwrap();
    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].base.as_deref(), Some(base.as_str()));
    assert!(
        stashes[0].message.starts_with("WIP on main"),
        "{}",
        stashes[0].message
    );
    assert!(stashes[0].time > 0);
}

#[test]
fn pop_apply_and_drop_by_commit_id() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "first\n");
    cli.stash_push(Some("first")).unwrap();
    repo.write("a.txt", "second\n");
    cli.stash_push(Some("second")).unwrap();
    let stashes = backend(&repo).stashes().unwrap();
    let (newest, older) = (stashes[0].id.clone(), stashes[1].id.clone());

    // The older stash is stash@{1}; it is found by id.
    cli.stash_apply(&older).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "first\n"
    );
    assert_eq!(backend(&repo).stashes().unwrap().len(), 2);
    repo.git(&["checkout", "--quiet", "--", "a.txt"]);

    cli.stash_drop(&older).unwrap();
    let left: Vec<_> = backend(&repo)
        .stashes()
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(left, std::slice::from_ref(&newest));

    cli.stash_pop(&newest).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "second\n"
    );
    assert!(backend(&repo).stashes().unwrap().is_empty());
}

#[test]
fn pop_with_conflict_keeps_the_stash() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "stashed\n");
    cli.stash_push(None).unwrap();
    repo.commit("a.txt", "committed\n", "Conflicting change");
    let id = backend(&repo).stashes().unwrap()[0].id.clone();

    let error = cli.stash_pop(&id).unwrap_err();
    assert!(matches!(error, Error::Command { .. }), "{error:?}");
    assert_eq!(backend(&repo).stashes().unwrap().len(), 1);
}

#[test]
fn unknown_stash_is_not_found() {
    let repo = isolated();
    let error = CliBackend::new(repo.path())
        .stash_pop("0123456789012345678901234567890123456789")
        .unwrap_err();
    assert!(matches!(error, Error::NotFound(_)));
}
