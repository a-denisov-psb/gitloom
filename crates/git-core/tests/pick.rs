// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    cherry_pick, commit_empty, revert, CliBackend, GitBackend as _, GitWriter as _, GixBackend,
    MergeOutcome, Operation, RepoState,
};
use support::TestRepo;

fn repo() -> TestRepo {
    let repo = TestRepo::new();
    // Commits made by the backend use the repository identity.
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    repo.commit("a.txt", "base\n", "base");
    repo
}

fn cli(repo: &TestRepo) -> CliBackend {
    CliBackend::new(repo.path())
}

fn state(repo: &TestRepo) -> RepoState {
    GixBackend::discover(repo.path()).unwrap().state().unwrap()
}

fn subjects(repo: &TestRepo, count: usize) -> Vec<String> {
    repo.git(&["log", "--format=%s", "-n", &count.to_string()])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `topic` with commits "one" (one.txt), "two" (two.txt), "three"
/// (a.txt → topic); `main` checked out. Returns their ids oldest first.
fn topic(repo: &TestRepo) -> Vec<String> {
    repo.branch("topic");
    repo.checkout("topic");
    let ids = vec![
        repo.commit("one.txt", "one\n", "one"),
        repo.commit("two.txt", "two\n", "two"),
        repo.commit("a.txt", "topic\n", "three"),
    ];
    repo.checkout("main");
    ids
}

#[test]
fn cherry_pick_one_commit() {
    let repo = repo();
    let ids = topic(&repo);
    assert_eq!(
        cherry_pick(&cli(&repo), &ids[1..2]).unwrap(),
        MergeOutcome::Done
    );
    assert_eq!(subjects(&repo, 2), ["two", "base"]);
    assert_eq!(repo.git(&["log", "-1", "--format=%an"]), "Test Author");
}

#[test]
fn cherry_pick_several_commits_in_order() {
    let repo = repo();
    let ids = topic(&repo);
    cherry_pick(&cli(&repo), &[ids[0].clone(), ids[2].clone()]).unwrap();
    assert_eq!(subjects(&repo, 3), ["three", "one", "base"]);
}

#[test]
fn conflicting_cherry_pick_stops_and_continues() {
    let repo = repo();
    let ids = topic(&repo);
    repo.commit("a.txt", "main\n", "main change");
    assert_eq!(
        cherry_pick(&cli(&repo), &ids[1..]).unwrap(),
        MergeOutcome::Conflicts
    );
    assert!(matches!(state(&repo), RepoState::CherryPicking { .. }));
    cli(&repo).abort_operation(Operation::CherryPick).unwrap();
    assert_eq!(subjects(&repo, 1), ["main change"]);
}

#[test]
fn empty_cherry_pick_can_be_skipped_or_committed() {
    for commit in [false, true] {
        let repo = repo();
        let ids = topic(&repo);
        // main already has the change of "one".
        repo.commit("one.txt", "one\n", "same as one");
        assert_eq!(
            cherry_pick(&cli(&repo), &ids[..2]).unwrap(),
            MergeOutcome::Conflicts
        );
        assert!(matches!(state(&repo), RepoState::CherryPicking { .. }));
        assert_eq!(repo.git(&["status", "--porcelain"]), "");
        if commit {
            // The empty commit is kept and "two" follows.
            assert_eq!(
                commit_empty(&cli(&repo), Operation::CherryPick).unwrap(),
                MergeOutcome::Done
            );
            assert_eq!(subjects(&repo, 3), ["two", "one", "same as one"]);
        } else {
            cli(&repo).skip_operation(Operation::CherryPick).unwrap();
            assert_eq!(subjects(&repo, 2), ["two", "same as one"]);
        }
        assert_eq!(state(&repo), RepoState::Clean);
    }
}

#[test]
fn revert_a_commit() {
    let repo = repo();
    let change = repo.commit("a.txt", "changed\n", "change");
    assert_eq!(
        revert(&cli(&repo), &change, None).unwrap(),
        MergeOutcome::Done
    );
    assert_eq!(subjects(&repo, 1), ["Revert \"change\""]);
    assert_eq!(repo.git(&["show", "HEAD:a.txt"]), "base");
}

#[test]
fn revert_of_a_merge_needs_the_mainline() {
    let repo = repo();
    topic(&repo);
    let merge = repo.merge("topic", "merge topic");
    assert!(revert(&cli(&repo), &merge, None).is_err());
    assert_eq!(state(&repo), RepoState::Clean);
    revert(&cli(&repo), &merge, Some(1)).unwrap();
    assert_eq!(repo.git(&["show", "HEAD:a.txt"]), "base");
    assert!(!repo.path().join("one.txt").exists());
}

#[test]
fn conflicting_revert_stops() {
    let repo = repo();
    let change = repo.commit("a.txt", "changed\n", "change");
    repo.commit("a.txt", "changed again\n", "again");
    assert_eq!(
        revert(&cli(&repo), &change, None).unwrap(),
        MergeOutcome::Conflicts
    );
    assert!(matches!(state(&repo), RepoState::Reverting { .. }));
}
