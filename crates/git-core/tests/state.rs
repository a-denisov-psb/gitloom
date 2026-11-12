// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{locked_file, CliBackend, GitBackend, GitWriter, GixBackend, RepoState};
use support::TestRepo;

fn state(repo: &TestRepo) -> RepoState {
    GixBackend::discover(repo.path()).unwrap().state().unwrap()
}

fn merge_message(repo: &TestRepo) -> Option<String> {
    GixBackend::discover(repo.path())
        .unwrap()
        .merge_message()
        .unwrap()
}

/// `main` and `topic` both change `a.txt` on top of a common base.
fn diverged() -> (TestRepo, String) {
    let repo = TestRepo::new();
    repo.commit("a.txt", "base\n", "Base");
    repo.branch("topic");
    repo.commit("a.txt", "main\n", "Main change");
    repo.checkout("topic");
    let topic = repo.commit("a.txt", "topic\n", "Topic change");
    repo.checkout("main");
    (repo, topic)
}

#[test]
fn clean_repository() {
    let (repo, _) = diverged();
    assert_eq!(state(&repo), RepoState::Clean);
    assert_eq!(merge_message(&repo), None);
}

#[test]
fn conflicting_merge() {
    let (repo, topic) = diverged();
    assert!(!repo.try_git(&["merge", "topic"]));
    assert_eq!(state(&repo), RepoState::Merging { merge_head: topic });
    assert!(merge_message(&repo).is_some_and(|m| m.starts_with("Merge branch 'topic'")));

    repo.git(&["merge", "--abort"]);
    assert_eq!(state(&repo), RepoState::Clean);
}

#[test]
fn conflicting_rebase_with_both_backends() {
    for backend in ["--merge", "--apply"] {
        let (repo, _) = diverged();
        repo.checkout("topic");
        let onto = repo.git(&["rev-parse", "main"]);
        let commit = repo.git(&["rev-parse", "topic"]);
        assert!(!repo.try_git(&["rebase", backend, "main"]));
        assert_eq!(
            state(&repo),
            RepoState::Rebasing {
                branch: Some("topic".into()),
                onto: Some(onto),
                step: Some((1, 1)),
                commit: Some(commit),
            },
            "{backend}"
        );
        // The merge backend prepares the message for `--continue`; the
        // apply backend commits the patch message without asking.
        let expected =
            (backend == "--merge").then(|| "Topic change\n\n# Conflicts:\n#\ta.txt\n".to_owned());
        assert_eq!(merge_message(&repo), expected, "{backend}");
    }
}

#[test]
fn conflicting_cherry_pick_and_revert() {
    let (repo, topic) = diverged();
    assert!(!repo.try_git(&["cherry-pick", &topic]));
    assert_eq!(
        state(&repo),
        RepoState::CherryPicking {
            commit: Some(topic.clone())
        }
    );
    assert!(merge_message(&repo).is_some_and(|m| m.starts_with("Topic change")));
    repo.git(&["cherry-pick", "--abort"]);

    // Reverting the base conflicts with the later change on main.
    let base = repo.git(&["rev-parse", "main~1"]);
    assert!(!repo.try_git(&["revert", "--no-edit", &base]));
    assert_eq!(state(&repo), RepoState::Reverting { commit: Some(base) });
    assert!(merge_message(&repo).is_some_and(|m| m.starts_with("Revert \"Base\"")));
}

#[test]
fn index_lock_errors_name_the_lock_file() {
    let (repo, _) = diverged();
    repo.write("a.txt", "changed\n");
    let lock = repo.path().join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();

    let error = CliBackend::new(repo.path())
        .stage(&["a.txt".to_owned()])
        .unwrap_err()
        .to_string();
    let named = locked_file(&error).expect("lock error");
    assert!(named.ends_with("index.lock"), "{named}");
    assert!(lock.exists(), "the lock is never removed");
}
