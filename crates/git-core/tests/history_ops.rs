// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    current_branch, fast_forward_branch, hard_reset_with_snapshot, index_tree, merge, redo_history,
    reset, undo_history, CliBackend, HistoryChange, HistoryUndo, ResetMode,
};
use support::TestRepo;

fn repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    repo.commit("a.txt", "base\n", "base");
    repo
}

/// `main` and `topic` diverged without conflicts; `main` checked out.
fn diverged() -> TestRepo {
    let repo = repo();
    repo.branch("topic");
    repo.commit("m.txt", "m\n", "main");
    repo.checkout("topic");
    repo.commit("t.txt", "t\n", "topic");
    repo.checkout("main");
    repo
}

fn change(repo: &TestRepo, kind: HistoryUndo, before: String) -> HistoryChange {
    HistoryChange {
        label: "test".into(),
        kind,
        before,
        after: repo.head(),
        branch: current_branch(&CliBackend::new(repo.path())).unwrap(),
    }
}

fn status(repo: &TestRepo) -> String {
    repo.git(&["status", "--porcelain"])
}

#[test]
fn merge_is_undone_and_redone() {
    let repo = diverged();
    let cli = CliBackend::new(repo.path());
    let before = repo.head();
    merge(&cli, "refs/heads/topic", false).unwrap();
    let merged = change(&repo, HistoryUndo::Keep, before.clone());
    let merge_commit = merged.after.clone();

    let undone = undo_history(&cli, repo.path(), &merged).unwrap();
    assert_eq!(repo.head(), before);
    assert!(!repo.path().join("t.txt").exists());
    redo_history(&cli, repo.path(), &undone).unwrap();
    assert_eq!(repo.head(), merge_commit);
    assert!(repo.path().join("t.txt").exists());
    assert_eq!(status(&repo), "");
}

#[test]
fn undo_refuses_when_head_or_branch_changed() {
    let repo = diverged();
    let cli = CliBackend::new(repo.path());
    let before = repo.head();
    merge(&cli, "refs/heads/topic", false).unwrap();
    let merged = change(&repo, HistoryUndo::Keep, before);
    repo.commit("later.txt", "later\n", "later");
    let head = repo.head();
    assert!(undo_history(&cli, repo.path(), &merged).is_err());
    assert_eq!(repo.head(), head);

    // Same commit, other branch checked out.
    repo.git(&["switch", "--quiet", "-c", "elsewhere", &merged.after]);
    assert!(undo_history(&cli, repo.path(), &merged).is_err());
}

#[test]
fn undo_keeps_conflicting_local_changes() {
    let repo = diverged();
    let cli = CliBackend::new(repo.path());
    let before = repo.head();
    merge(&cli, "refs/heads/topic", false).unwrap();
    let merged = change(&repo, HistoryUndo::Keep, before);
    // t.txt only exists after the merge; editing it blocks `reset --keep`.
    repo.write("t.txt", "local edit\n");
    assert!(undo_history(&cli, repo.path(), &merged).is_err());
    assert_eq!(repo.head(), merged.after);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("t.txt")).unwrap(),
        "local edit\n"
    );
}

#[test]
fn fast_forward_of_another_branch_is_undone_and_redone() {
    let repo = repo();
    repo.branch("behind");
    let main = repo.commit("b.txt", "b\n", "ahead");
    let cli = CliBackend::new(repo.path());
    let before = repo.git(&["rev-parse", "behind"]);
    fast_forward_branch(&cli, "behind", "refs/heads/main").unwrap();
    let moved = HistoryChange {
        label: "ff".into(),
        kind: HistoryUndo::MoveBranch {
            name: "behind".into(),
        },
        before: before.clone(),
        after: main.clone(),
        branch: Some("main".into()),
    };
    undo_history(&cli, repo.path(), &moved).unwrap();
    assert_eq!(repo.git(&["rev-parse", "behind"]), before);
    redo_history(&cli, repo.path(), &moved).unwrap();
    assert_eq!(repo.git(&["rev-parse", "behind"]), main);
}

#[test]
fn soft_and_mixed_resets_restore_the_index() {
    for mode in [ResetMode::Soft, ResetMode::Mixed] {
        let repo = repo();
        let base = repo.head();
        repo.commit("b.txt", "b\n", "second");
        repo.write("a.txt", "staged\n");
        repo.git(&["add", "a.txt"]);
        let cli = CliBackend::new(repo.path());
        let before = repo.head();
        let tree = index_tree(&cli).unwrap();
        let status_before = status(&repo);
        reset(&cli, &base, mode).unwrap();
        let entry = change(
            &repo,
            HistoryUndo::Reset {
                unmerged: Vec::new(),
                mode,
                index_tree: tree,
            },
            before.clone(),
        );
        undo_history(&cli, repo.path(), &entry).unwrap();
        assert_eq!(repo.head(), before, "{mode:?}");
        assert_eq!(status(&repo), status_before, "{mode:?}");
        redo_history(&cli, repo.path(), &entry).unwrap();
        assert_eq!(repo.head(), base, "{mode:?}");
    }
}

#[test]
fn hard_reset_is_undone_and_redone_with_a_fresh_snapshot() {
    let repo = repo();
    let base = repo.head();
    repo.commit("b.txt", "b\n", "second");
    repo.write("a.txt", "local\n");
    let cli = CliBackend::new(repo.path());
    let before = repo.head();
    let snapshot = hard_reset_with_snapshot(&cli, repo.path(), &base).unwrap();
    let entry = change(&repo, HistoryUndo::HardReset(snapshot), before.clone());

    let undone = undo_history(&cli, repo.path(), &entry).unwrap();
    assert_eq!(repo.head(), before);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "local\n"
    );
    let redone = redo_history(&cli, repo.path(), &undone).unwrap();
    assert_eq!(repo.head(), base);
    assert_eq!(status(&repo), "");
    // The new snapshot undoes the redo again.
    undo_history(&cli, repo.path(), &redone).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "local\n"
    );
}

#[test]
fn rebase_cherry_pick_and_revert_are_undone_and_redone() {
    use git_core::{cherry_pick, rebase, revert};
    for op in ["rebase", "cherry-pick", "revert"] {
        let repo = diverged();
        let cli = CliBackend::new(repo.path());
        if op == "rebase" {
            repo.checkout("topic");
        }
        let before = repo.head();
        match op {
            "rebase" => {
                rebase(&cli, "refs/heads/main", false).unwrap();
            }
            "cherry-pick" => {
                let topic = repo.git(&["rev-parse", "topic"]);
                cherry_pick(&cli, &[topic]).unwrap();
            }
            _ => {
                revert(&cli, &before, None).unwrap();
            }
        }
        let entry = change(&repo, HistoryUndo::Keep, before.clone());
        assert_ne!(entry.after, before, "{op}");
        let undone = undo_history(&cli, repo.path(), &entry).unwrap();
        assert_eq!(repo.head(), before, "{op}");
        redo_history(&cli, repo.path(), &undone).unwrap();
        assert_eq!(repo.head(), entry.after, "{op}");
        assert_eq!(status(&repo), "", "{op}");
    }
}
