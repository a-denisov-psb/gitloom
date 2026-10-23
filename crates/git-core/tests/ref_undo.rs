// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{CliBackend, Error, GitWriter, HeadPosition, RefChange};
use support::TestRepo;

fn repo() -> (TestRepo, CliBackend) {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.commit("a.txt", "a\n", "A");
    repo.branch("topic");
    let cli = CliBackend::new(repo.path());
    (repo, cli)
}

fn head(cli: &CliBackend) -> HeadPosition {
    git_core::head_position(cli).unwrap().unwrap()
}

fn exists(repo: &TestRepo, reference: &str) -> bool {
    repo.try_git(&["rev-parse", "--verify", "--quiet", reference])
}

fn assert_changed(result: git_core::Result<()>) {
    match result {
        Err(Error::Backend(message)) => {
            assert!(message.contains("repository changed"), "{message}")
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn checkout_undo_and_redo() {
    let (repo, cli) = repo();
    let from = head(&cli);
    cli.switch_branch("topic").unwrap();
    let change = RefChange::Checkout {
        from: from.clone(),
        to: head(&cli),
    };
    change.apply(&cli, true).unwrap();
    assert_eq!(head(&cli), HeadPosition::Branch("main".into()));
    change.apply(&cli, false).unwrap();
    assert_eq!(head(&cli), HeadPosition::Branch("topic".into()));

    // HEAD moved elsewhere in the meantime: refused.
    let id = repo.head();
    cli.switch_detached(&id).unwrap();
    assert_changed(change.apply(&cli, true));
}

#[test]
fn create_branch_undo_and_redo() {
    let (repo, cli) = repo();
    let from = head(&cli);
    let target = repo.head();
    cli.create_branch("feature", &target).unwrap();
    let change = RefChange::CreateBranch {
        name: "feature".into(),
        target: target.clone(),
        from,
    };
    change.apply(&cli, true).unwrap();
    assert!(!exists(&repo, "refs/heads/feature"));
    assert_eq!(head(&cli), HeadPosition::Branch("main".into()));
    change.apply(&cli, false).unwrap();
    assert_eq!(head(&cli), HeadPosition::Branch("feature".into()));

    // A commit on the new branch makes undo refuse (it would be lost).
    repo.commit("b.txt", "b\n", "On feature");
    assert_changed(change.apply(&cli, true));
}

#[test]
fn delete_branch_undo_restores_target_and_upstream() {
    let (repo, cli) = repo();
    repo.fake_upstream("origin", "topic", "topic");
    let target = repo.git(&["rev-parse", "topic"]);
    let upstream = git_core::branch_upstream(&cli, "topic").unwrap();
    assert_eq!(upstream.as_deref(), Some("origin/topic"));
    cli.delete_branch("topic", true).unwrap();
    let change = RefChange::DeleteBranch {
        name: "topic".into(),
        target: target.clone(),
        upstream,
    };
    change.apply(&cli, true).unwrap();
    assert_eq!(repo.git(&["rev-parse", "topic"]), target);
    assert_eq!(
        git_core::branch_upstream(&cli, "topic").unwrap().as_deref(),
        Some("origin/topic")
    );
    change.apply(&cli, false).unwrap();
    assert!(!exists(&repo, "refs/heads/topic"));
}

#[test]
fn rename_branch_undo_and_redo() {
    let (repo, cli) = repo();
    cli.rename_branch("topic", "renamed").unwrap();
    let change = RefChange::RenameBranch {
        old: "topic".into(),
        new: "renamed".into(),
    };
    change.apply(&cli, true).unwrap();
    assert!(exists(&repo, "refs/heads/topic") && !exists(&repo, "refs/heads/renamed"));
    change.apply(&cli, false).unwrap();
    assert!(exists(&repo, "refs/heads/renamed"));

    // Someone created the old name again: undo refused.
    repo.branch("topic");
    assert_changed(change.apply(&cli, true));
}

#[test]
fn tag_undo_and_redo_keep_the_message() {
    let (repo, cli) = repo();
    let target = repo.head();
    cli.create_tag("v1", &target, Some("Release 1")).unwrap();
    let message = git_core::tag_message(&cli, "v1").unwrap();
    assert_eq!(message.as_deref(), Some("Release 1"));
    cli.delete_tag("v1").unwrap();
    let delete = RefChange::DeleteTag {
        name: "v1".into(),
        target: target.clone(),
        message,
    };
    delete.apply(&cli, true).unwrap();
    assert_eq!(
        git_core::tag_message(&cli, "v1").unwrap().as_deref(),
        Some("Release 1")
    );
    delete.apply(&cli, false).unwrap();
    assert!(!exists(&repo, "refs/tags/v1"));

    cli.create_tag("v2", &target, None).unwrap();
    assert_eq!(
        git_core::tag_message(&cli, "v2").unwrap(),
        None,
        "lightweight"
    );
    let create = RefChange::CreateTag {
        name: "v2".into(),
        target,
        message: None,
    };
    create.apply(&cli, true).unwrap();
    assert!(!exists(&repo, "refs/tags/v2"));
    create.apply(&cli, false).unwrap();
    assert!(exists(&repo, "refs/tags/v2"));
}
