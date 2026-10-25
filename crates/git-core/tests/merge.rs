// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    fast_forward_branch, fast_forward_head, merge, merge_blocked, rebase, rebase_blocked,
    CliBackend, Error, GitBackend as _, GitWriter as _, GixBackend, MergeOutcome, RepoState,
};
use support::TestRepo;

fn repo() -> TestRepo {
    let repo = TestRepo::new();
    // Merge commits made by the backend use the repository identity.
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

fn rev(repo: &TestRepo, name: &str) -> String {
    repo.git(&["rev-parse", name])
}

#[test]
fn merge_fast_forwards_when_possible() {
    let repo = repo();
    repo.branch("topic");
    repo.checkout("topic");
    let tip = repo.commit("b.txt", "b\n", "topic");
    repo.checkout("main");
    assert_eq!(
        merge(&cli(&repo), "topic", false).unwrap(),
        MergeOutcome::Done
    );
    assert_eq!(repo.head(), tip);
}

#[test]
fn merge_creates_a_merge_commit_for_diverged_branches() {
    let repo = repo();
    repo.branch("topic");
    let main = repo.commit("m.txt", "m\n", "main");
    repo.checkout("topic");
    let topic = repo.commit("b.txt", "b\n", "topic");
    repo.checkout("main");
    assert_eq!(
        merge(&cli(&repo), "refs/heads/topic", false).unwrap(),
        MergeOutcome::Done
    );
    assert_eq!(repo.parents("HEAD"), [main, topic]);
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]),
        "Merge branch 'topic'"
    );

    // Remote-tracking branches get Git's remote message.
    repo.checkout("topic");
    repo.commit("c.txt", "c\n", "topic 2");
    repo.git(&["update-ref", "refs/remotes/origin/other", "HEAD"]);
    repo.checkout("main");
    merge(&cli(&repo), "refs/remotes/origin/other", false).unwrap();
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]),
        "Merge remote-tracking branch 'origin/other'"
    );
}

#[test]
fn conflicting_merge_stops_in_progress() {
    let repo = repo();
    repo.branch("topic");
    repo.commit("a.txt", "main\n", "main");
    repo.checkout("topic");
    repo.commit("a.txt", "topic\n", "topic");
    repo.checkout("main");
    assert_eq!(
        merge(&cli(&repo), "topic", false).unwrap(),
        MergeOutcome::Conflicts
    );
    assert!(matches!(state(&repo), RepoState::Merging { .. }));
}

#[test]
fn local_changes_block_the_merge() {
    let repo = repo();
    repo.branch("topic");
    repo.checkout("topic");
    repo.commit("a.txt", "topic\n", "topic");
    repo.checkout("main");
    let main = repo.head();
    repo.write("a.txt", "local\n");
    match merge(&cli(&repo), "topic", false) {
        Err(Error::Command { stderr, .. }) => assert!(merge_blocked(&stderr), "{stderr}"),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(state(&repo), RepoState::Clean);
    assert_eq!(repo.head(), main);
}

#[test]
fn autostash_reapplies_changes_after_the_merge() {
    let repo = repo();
    repo.commit("b.txt", "one\ntwo\nthree\n", "b");
    repo.branch("topic");
    repo.checkout("topic");
    let tip = repo.commit("b.txt", "ONE\ntwo\nthree\n", "topic");
    repo.checkout("main");
    // Overlaps the merged file without conflicting: Git refuses without autostash.
    repo.write("b.txt", "one\ntwo\nTHREE\n");
    let error = fast_forward_head(&cli(&repo), "topic", false).unwrap_err();
    assert!(merge_blocked(&error.to_string()), "{error}");
    fast_forward_head(&cli(&repo), "topic", true).unwrap();
    assert_eq!(repo.head(), tip);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("b.txt")).unwrap(),
        "ONE\ntwo\nTHREE\n"
    );
}

#[test]
fn fast_forward_head_refuses_diverged_branches() {
    let repo = repo();
    repo.branch("topic");
    let main = repo.commit("m.txt", "m\n", "main");
    repo.checkout("topic");
    repo.commit("b.txt", "b\n", "topic");
    repo.checkout("main");
    assert!(fast_forward_head(&cli(&repo), "topic", false).is_err());
    assert_eq!(repo.head(), main);
}

#[test]
fn fast_forward_of_a_branch_that_is_not_checked_out() {
    let repo = repo();
    repo.branch("behind");
    let main = repo.commit("m.txt", "m\n", "main");
    fast_forward_branch(&cli(&repo), "behind", "refs/heads/main").unwrap();
    assert_eq!(rev(&repo, "behind"), main);
    assert!(!repo.path().join(".git/FETCH_HEAD").exists());

    // Diverged: refused, branch unchanged.
    repo.checkout("behind");
    let other = repo.commit("o.txt", "o\n", "other");
    repo.checkout("main");
    repo.commit("n.txt", "n\n", "main 2");
    assert!(fast_forward_branch(&cli(&repo), "behind", "refs/heads/main").is_err());
    assert_eq!(rev(&repo, "behind"), other);
}

#[test]
fn fast_forward_branch_refuses_the_checked_out_branch() {
    let repo = repo();
    repo.branch("ahead");
    repo.checkout("ahead");
    let ahead = repo.commit("b.txt", "b\n", "ahead");
    repo.checkout("main");
    let main = repo.head();
    assert!(fast_forward_branch(&cli(&repo), "main", "refs/heads/ahead").is_err());
    assert_eq!(repo.head(), main);
    assert_eq!(rev(&repo, "ahead"), ahead);
}

/// `topic` (checked out) has "topic 1" (b.txt) and "topic 2" (a.txt);
/// `main` changed a.txt after the fork.
fn rebase_setup() -> TestRepo {
    let repo = repo();
    repo.branch("topic");
    repo.commit("a.txt", "main\n", "main");
    repo.checkout("topic");
    repo.commit("b.txt", "b\n", "topic 1");
    repo.commit("c.txt", "c\n", "topic 2");
    repo
}

fn subjects(repo: &TestRepo, range: &str) -> Vec<String> {
    repo.git(&["log", "--format=%s", range])
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn rebase_replays_commits_onto_the_target() {
    let repo = rebase_setup();
    assert_eq!(
        rebase(&cli(&repo), "refs/heads/main", false).unwrap(),
        MergeOutcome::Done
    );
    assert_eq!(subjects(&repo, "main..topic"), ["topic 2", "topic 1"]);
    assert_eq!(repo.parents("HEAD~1"), [rev(&repo, "main")]);
    assert_eq!(state(&repo), RepoState::Clean);
}

#[test]
fn conflicting_rebase_stops_and_can_be_aborted() {
    let repo = rebase_setup();
    repo.commit("a.txt", "topic\n", "topic 3");
    let before = repo.head();
    assert_eq!(
        rebase(&cli(&repo), "main", false).unwrap(),
        MergeOutcome::Conflicts
    );
    assert!(matches!(state(&repo), RepoState::Rebasing { .. }));
    cli(&repo)
        .abort_operation(git_core::Operation::Rebase)
        .unwrap();
    assert_eq!(state(&repo), RepoState::Clean);
    assert_eq!(repo.head(), before);
}

#[test]
fn local_changes_block_the_rebase_unless_autostashed() {
    let repo = rebase_setup();
    repo.write("b.txt", "local\n");
    match rebase(&cli(&repo), "main", false) {
        Err(Error::Command { stderr, .. }) => assert!(rebase_blocked(&stderr), "{stderr}"),
        other => panic!("unexpected {other:?}"),
    }
    repo.git(&["add", "b.txt"]);
    let error = rebase(&cli(&repo), "main", false).unwrap_err();
    assert!(rebase_blocked(&error.to_string()), "{error}");

    assert_eq!(
        rebase(&cli(&repo), "main", true).unwrap(),
        MergeOutcome::Done
    );
    assert_eq!(repo.parents("HEAD~1"), [rev(&repo, "main")]);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("b.txt")).unwrap(),
        "local\n"
    );
}
