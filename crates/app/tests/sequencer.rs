//! Continue, skip and abort of merge, rebase, cherry-pick and revert (M4-03),
//! with the real binary answering Git's message editor (ADR-021).

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../../git-core/tests/support/mod.rs"]
mod support;

use git_core::{CliBackend, Error, GitBackend as _, GitWriter, GixBackend, Operation, RepoState};
use support::TestRepo;

/// `main` and `topic` both change `a.txt` after a common base; `main` is
/// checked out. Returns the repository.
fn diverged() -> TestRepo {
    let repo = TestRepo::new();
    // Commands run by the backend commit with the repository identity.
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    repo.commit("a.txt", "base\n", "base");
    repo.branch("topic");
    repo.commit("a.txt", "main\n", "main change");
    repo.checkout("topic");
    repo.commit("b.txt", "b\n", "topic first");
    repo.commit("a.txt", "topic\n", "topic change");
    repo.checkout("main");
    repo
}

fn backend(repo: &TestRepo) -> CliBackend {
    CliBackend::new(repo.path()).with_editor(env!("CARGO_BIN_EXE_gitloom"))
}

fn state(repo: &TestRepo) -> RepoState {
    GixBackend::discover(repo.path()).unwrap().state().unwrap()
}

/// Resolves the conflict in `a.txt` and stages it.
fn resolve(repo: &TestRepo) {
    repo.write("a.txt", "resolved\n");
    repo.git(&["add", "a.txt"]);
}

fn message(repo: &TestRepo, rev: &str) -> String {
    repo.git(&["log", "-1", "--format=%B", rev])
}

/// Starts `operation` so that it stops with a conflict in `a.txt`.
fn start(repo: &TestRepo, operation: Operation) {
    let args: &[&str] = match operation {
        Operation::Merge => &["merge", "topic"],
        Operation::Rebase => {
            repo.checkout("topic");
            &["rebase", "main"]
        }
        // Picks "topic first" (clean) and then "topic change" (conflict).
        Operation::CherryPick => &["cherry-pick", "main..topic"],
        // Reverting "main change" on top of an edit of the same line.
        Operation::Revert => {
            repo.commit("a.txt", "main again\n", "main again");
            &["revert", "--no-edit", "HEAD~1"]
        }
    };
    assert!(!repo.try_git(args), "{operation:?} should stop");
    assert_eq!(state(repo).kind(), Some(operation));
}

#[test]
fn continue_uses_the_prepared_message() {
    for operation in [
        Operation::Merge,
        Operation::Rebase,
        Operation::CherryPick,
        Operation::Revert,
    ] {
        let repo = diverged();
        start(&repo, operation);
        let cli = backend(&repo);
        resolve(&repo);
        cli.continue_operation(operation, "Resolved in Gitloom\n\nbody\n")
            .unwrap();
        assert_eq!(state(&repo), RepoState::Clean, "{operation:?}");
        assert_eq!(
            message(&repo, "HEAD"),
            "Resolved in Gitloom\n\nbody",
            "{operation:?}"
        );
        assert_eq!(repo.git(&["show", "HEAD:a.txt"]), "resolved");
        if operation == Operation::Merge {
            assert_eq!(repo.parents("HEAD").len(), 2);
        }
    }
}

#[test]
fn continue_with_unresolved_conflicts_keeps_the_state() {
    for operation in [
        Operation::Merge,
        Operation::Rebase,
        Operation::CherryPick,
        Operation::Revert,
    ] {
        let repo = diverged();
        start(&repo, operation);
        let result = backend(&repo).continue_operation(operation, "message\n");
        assert!(
            matches!(result, Err(Error::Command { .. })),
            "{operation:?}: {result:?}"
        );
        assert_eq!(state(&repo).kind(), Some(operation));
    }
}

#[test]
fn skip_drops_the_conflicting_commit() {
    for operation in [Operation::Rebase, Operation::CherryPick] {
        let repo = diverged();
        start(&repo, operation);
        backend(&repo).skip_operation(operation).unwrap();
        assert_eq!(state(&repo), RepoState::Clean, "{operation:?}");
        // "topic first" was applied, "topic change" skipped.
        assert_eq!(message(&repo, "HEAD"), "topic first", "{operation:?}");
        assert_eq!(message(&repo, "HEAD~1"), "main change", "{operation:?}");
        assert_eq!(repo.git(&["show", "HEAD:a.txt"]), "main");
    }
}

#[test]
fn skip_is_refused_for_merge_and_revert() {
    for operation in [Operation::Merge, Operation::Revert] {
        let repo = diverged();
        start(&repo, operation);
        let result = backend(&repo).skip_operation(operation);
        assert!(matches!(result, Err(Error::Backend(_))), "{result:?}");
        assert_eq!(state(&repo).kind(), Some(operation));
    }
}

#[test]
fn abort_restores_the_previous_state() {
    for operation in [
        Operation::Merge,
        Operation::Rebase,
        Operation::CherryPick,
        Operation::Revert,
    ] {
        let repo = diverged();
        let branch_before = |repo: &TestRepo| match operation {
            Operation::Rebase => repo.git(&["rev-parse", "topic"]),
            _ => repo.git(&["rev-parse", "main"]),
        };
        let before = if operation == Operation::Revert {
            None
        } else {
            Some(branch_before(&repo))
        };
        start(&repo, operation);
        // A revert test commits once more before stopping.
        let before = before.unwrap_or_else(|| repo.head());
        backend(&repo).abort_operation(operation).unwrap();
        assert_eq!(state(&repo), RepoState::Clean, "{operation:?}");
        assert_eq!(repo.head(), before, "{operation:?}");
        assert_eq!(repo.git(&["status", "--porcelain"]), "", "{operation:?}");
    }
}

#[test]
fn resolving_in_the_conflict_model_then_continuing() {
    for operation in [Operation::Merge, Operation::Rebase, Operation::CherryPick] {
        let repo = diverged();
        start(&repo, operation);
        let cli = backend(&repo);
        let loaded = git_core::load_conflict(&cli, "a.txt").unwrap();
        let git_core::ConflictContent::Text(text) = loaded.content else {
            panic!("expected a text conflict");
        };
        let mut picks = git_core::Picks::new(&text);
        picks.take_all(git_core::Side::Theirs);
        let output = text.output(&picks, ("ours", "theirs"));
        git_core::resolve_conflict(&cli, "a.txt", output.as_bytes()).unwrap();
        cli.continue_operation(operation, "Resolved\n").unwrap();
        assert_eq!(state(&repo), RepoState::Clean, "{operation:?}");
        // In a rebase "theirs" is the replayed commit; in a merge or
        // cherry-pick it is the merged or picked one.
        assert_eq!(repo.git(&["show", "HEAD:a.txt"]), "topic", "{operation:?}");
    }
}
