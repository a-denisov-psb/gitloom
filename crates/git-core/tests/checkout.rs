// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{checkout_blocked, CliBackend, Error, GitBackend, GitWriter, GixBackend, Head};
use support::TestRepo;

fn head(repo: &TestRepo) -> Head {
    GixBackend::discover(repo.path()).unwrap().head().unwrap()
}

/// `main` with commit A, `topic` with commit B on top changing a.txt.
fn two_branches() -> TestRepo {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "A");
    repo.branch("topic");
    repo.checkout("topic");
    repo.commit("a.txt", "topic\n", "B");
    repo.checkout("main");
    repo
}

#[test]
fn switch_to_a_local_branch() {
    let repo = two_branches();
    CliBackend::new(repo.path()).switch_branch("topic").unwrap();
    assert!(matches!(head(&repo), Head::Branch { name, .. } if name == "topic"));
}

#[test]
fn remote_branch_becomes_a_tracking_branch() {
    let repo = two_branches();
    repo.fake_upstream("origin", "main", "main");
    repo.git(&["update-ref", "refs/remotes/origin/feature", "topic"]);
    CliBackend::new(repo.path())
        .switch_tracking("feature", "origin/feature")
        .unwrap();
    assert!(matches!(head(&repo), Head::Branch { name, .. } if name == "feature"));
    assert_eq!(
        repo.git(&["rev-parse", "--abbrev-ref", "feature@{upstream}"]),
        "origin/feature"
    );
}

#[test]
fn commit_checkout_detaches_head() {
    let repo = two_branches();
    let id = repo.git(&["rev-parse", "topic"]);
    CliBackend::new(repo.path()).switch_detached(&id).unwrap();
    assert_eq!(head(&repo), Head::Detached { target: id });
}

#[test]
fn conflicting_local_changes_block_until_stashed() {
    let repo = two_branches();
    repo.write("a.txt", "local edit\n");
    let cli = CliBackend::new(repo.path());
    match cli.switch_branch("topic") {
        Err(Error::Command { stderr, .. }) => assert!(checkout_blocked(&stderr), "{stderr}"),
        other => panic!("unexpected {other:?}"),
    }
    // "Stash and checkout": the stash stays, it is not popped.
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    cli.stash_push(Some("Gitloom: before checkout")).unwrap();
    cli.switch_branch("topic").unwrap();
    assert!(matches!(head(&repo), Head::Branch { name, .. } if name == "topic"));
    assert_eq!(
        GixBackend::discover(repo.path())
            .unwrap()
            .stashes()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn untracked_file_in_the_way_blocks_too() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "A");
    repo.branch("topic");
    repo.checkout("topic");
    repo.commit("new.txt", "tracked on topic\n", "B");
    repo.checkout("main");
    repo.write("new.txt", "untracked here\n");
    match CliBackend::new(repo.path()).switch_branch("topic") {
        Err(Error::Command { stderr, .. }) => assert!(checkout_blocked(&stderr), "{stderr}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn create_branch_at_a_commit_and_switch_to_it() {
    let repo = two_branches();
    let base = repo.git(&["rev-parse", "main"]);
    let cli = CliBackend::new(repo.path());
    cli.create_branch("feature/new", &base).unwrap();
    assert!(
        matches!(head(&repo), Head::Branch { name, target } if name == "feature/new" && target == base)
    );

    // Existing names and invalid names fail without changing HEAD.
    assert!(cli.create_branch("topic", &base).is_err());
    match cli.create_branch("bad..name", &base) {
        Err(Error::Backend(message)) => assert!(message.contains("not a valid branch name")),
        other => panic!("unexpected {other:?}"),
    }
    assert!(matches!(head(&repo), Head::Branch { name, .. } if name == "feature/new"));
}

#[test]
fn name_rules_agree_with_git() {
    let repo = TestRepo::new();
    for name in [
        "ok/name", "a..b", "a.lock", "-x", "a b", "x/", ".hidden", "fine-1.2",
    ] {
        let git_ok = repo.try_git(&["check-ref-format", "--branch", name]);
        assert_eq!(
            git_core::check_branch_name(name).is_none(),
            git_ok,
            "{name}"
        );
    }
}
