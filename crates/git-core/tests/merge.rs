// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    fast_forward_branch, fast_forward_head, merge, merge_blocked, CliBackend, Error,
    GitBackend as _, GixBackend, MergeOutcome, RepoState,
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
        MergeOutcome::Merged
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
        MergeOutcome::Merged
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
