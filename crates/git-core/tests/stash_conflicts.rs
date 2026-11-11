//! Stash pop/apply that stops in conflicts (M9-01) and its undo/redo
//! (M9-02, ADR-043).

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::collections::BTreeMap;
use std::path::Path;

use git_core::{
    apply_stash, apply_stash_recorded, redo_stash, stash_changed_since, undo_stash, unmerged_paths,
    CliBackend, Error, StashOutcome,
};
use support::TestRepo;

fn isolated() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.commit("a.txt", "a\nb\nc\n", "Base");
    repo
}

/// Every working tree file (outside `.git`) with its bytes.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy();
                out.insert(rel.replace('\\', "/"), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Everything undo must restore exactly: files, index entries, stash list.
#[derive(Debug, PartialEq, Eq)]
struct State {
    files: BTreeMap<String, Vec<u8>>,
    index: String,
    stashes: String,
    head: String,
}

fn state(repo: &TestRepo) -> State {
    State {
        files: files(repo.path()),
        index: repo.git(&["ls-files", "-s"]),
        stashes: repo.git(&["stash", "list", "--format=%H %gs"]),
        head: repo.head(),
    }
}

/// The maintainer's case (2026-11-11): a staged change is stashed, then a
/// merge changes the same lines, then the stash is popped.
fn stash_then_conflicting_merge(repo: &TestRepo) -> String {
    repo.git(&["switch", "--quiet", "-c", "topic"]);
    repo.commit("a.txt", "a\nB-topic\nc\n", "Topic");
    repo.checkout("main");
    repo.write("a.txt", "a\nB-stash\nc\n");
    repo.git(&["add", "a.txt"]);
    repo.git(&["stash", "push", "--quiet"]);
    repo.merge("topic", "Merge topic");
    repo.git(&["rev-parse", "stash@{0}"])
}

#[test]
fn pop_with_conflicts_keeps_the_stash_and_reports_conflicts() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);

    assert_eq!(
        apply_stash(&cli, &id, true).unwrap(),
        StashOutcome::Conflicts
    );
    assert_eq!(repo.git(&["rev-parse", "stash@{0}"]), id);
    assert_eq!(unmerged_paths(&cli).unwrap(), ["a.txt"]);
}

#[test]
fn apply_with_conflicts_reports_conflicts() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);

    assert_eq!(
        apply_stash(&cli, &id, false).unwrap(),
        StashOutcome::Conflicts
    );
    assert_eq!(repo.git(&["stash", "list", "--format=%H"]), id);
    assert_eq!(unmerged_paths(&cli).unwrap(), ["a.txt"]);
}

/// A second pop while the first one's conflicts are open: Git refuses
/// ("needs merge", newer versions add "could not write index"); Gitloom
/// refuses before running it and says why.
#[test]
fn pop_is_refused_while_files_are_conflicted() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);
    apply_stash(&cli, &id, true).unwrap();

    let error = apply_stash(&cli, &id, true).unwrap_err();
    assert!(error.to_string().contains("conflicted files"), "{error}");
}

#[test]
fn failure_without_conflicts_stays_an_error() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("new.txt", "stashed\n");
    repo.git(&["stash", "push", "--quiet", "--include-untracked"]);
    let id = repo.git(&["rev-parse", "stash@{0}"]);
    // The untracked file of the stash exists again: Git refuses.
    repo.write("new.txt", "in the way\n");

    let error = apply_stash(&cli, &id, true).unwrap_err();
    assert!(matches!(error, Error::Command { .. }), "{error:?}");
    assert_eq!(repo.git(&["rev-parse", "stash@{0}"]), id);
}

#[test]
fn clean_pop_is_applied_and_drops_the_stash() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "changed\n");
    repo.git(&["stash", "push", "--quiet"]);
    let id = repo.git(&["rev-parse", "stash@{0}"]);

    assert_eq!(apply_stash(&cli, &id, true).unwrap(), StashOutcome::Applied);
    assert_eq!(repo.git(&["stash", "list"]), "");
}

#[test]
fn undo_pop_with_conflicts_on_a_clean_tree() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);
    let before = state(&repo);

    let record = apply_stash_recorded(&cli, repo.path(), &id, true).unwrap();
    assert_eq!(record.outcome, StashOutcome::Conflicts);
    assert!(!record.dropped);
    assert_ne!(state(&repo), before);

    undo_stash(&cli, repo.path(), &record).unwrap();
    assert_eq!(state(&repo), before);
    assert!(unmerged_paths(&cli).unwrap().is_empty());
}

#[test]
fn undo_pop_with_conflicts_keeps_other_local_changes() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.commit("b.txt", "b\n", "B");
    let id = stash_then_conflicting_merge(&repo);
    // Unrelated local work: a modified, a staged and an untracked file.
    repo.write("b.txt", "local edit\n");
    repo.write("staged.txt", "staged\n");
    repo.git(&["add", "staged.txt"]);
    repo.write("dir/untracked.txt", "untracked\n");
    let before = state(&repo);

    let record = apply_stash_recorded(&cli, repo.path(), &id, true).unwrap();
    assert_eq!(record.outcome, StashOutcome::Conflicts);
    undo_stash(&cli, repo.path(), &record).unwrap();
    assert_eq!(state(&repo), before);
}

#[test]
fn undo_pop_of_a_stash_with_untracked_files() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "changed\n");
    repo.write("new/dir/file.txt", "new\n");
    repo.git(&["stash", "push", "--quiet", "--include-untracked"]);
    let id = repo.git(&["rev-parse", "stash@{0}"]);
    let before = state(&repo);

    let record = apply_stash_recorded(&cli, repo.path(), &id, true).unwrap();
    assert_eq!(record.outcome, StashOutcome::Applied);
    assert!(record.dropped);
    assert!(repo.path().join("new/dir/file.txt").exists());

    undo_stash(&cli, repo.path(), &record).unwrap();
    assert_eq!(state(&repo), before);
    assert!(
        !repo.path().join("new").exists(),
        "created folders are removed"
    );
}

#[test]
fn undo_successful_pop_restores_the_stash_at_its_position() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    for (file, message) in [
        ("x.txt", "oldest"),
        ("y.txt", "middle"),
        ("z.txt", "newest"),
    ] {
        repo.write(file, message);
        repo.git(&[
            "stash",
            "push",
            "--quiet",
            "--include-untracked",
            "-m",
            message,
        ]);
    }
    let middle = repo.git(&["rev-parse", "stash@{1}"]);
    let before = state(&repo);

    let record = apply_stash_recorded(&cli, repo.path(), &middle, true).unwrap();
    assert!(record.dropped);
    assert_eq!(record.position, 1);
    undo_stash(&cli, repo.path(), &record).unwrap();
    assert_eq!(state(&repo), before);
}

#[test]
fn undo_apply_with_conflicts() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);
    let before = state(&repo);

    let record = apply_stash_recorded(&cli, repo.path(), &id, false).unwrap();
    assert_eq!(record.outcome, StashOutcome::Conflicts);
    assert!(!record.dropped);
    undo_stash(&cli, repo.path(), &record).unwrap();
    assert_eq!(state(&repo), before);
}

#[test]
fn redo_applies_again_and_is_refused_when_the_stash_is_gone() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);

    let record = apply_stash_recorded(&cli, repo.path(), &id, true).unwrap();
    let after = state(&repo);
    undo_stash(&cli, repo.path(), &record).unwrap();
    let redone = redo_stash(&cli, repo.path(), &record).unwrap();
    assert_eq!(redone.outcome, StashOutcome::Conflicts);
    assert_eq!(state(&repo), after);

    undo_stash(&cli, repo.path(), &redone).unwrap();
    repo.git(&["stash", "drop", "--quiet"]);
    let error = redo_stash(&cli, repo.path(), &redone).unwrap_err();
    assert!(
        error.to_string().contains("no longer in the list"),
        "{error}"
    );
}

#[test]
fn editing_a_conflicted_file_is_detected_before_undo() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);
    let record = apply_stash_recorded(&cli, repo.path(), &id, true).unwrap();
    assert!(!stash_changed_since(&cli, repo.path(), &record).unwrap());

    repo.write("a.txt", "a\nresolved\nc\n");
    assert!(stash_changed_since(&cli, repo.path(), &record).unwrap());
}

#[test]
fn undo_is_refused_after_head_moved() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "changed\n");
    repo.git(&["stash", "push", "--quiet"]);
    let id = repo.git(&["rev-parse", "stash@{0}"]);
    let record = apply_stash_recorded(&cli, repo.path(), &id, true).unwrap();
    repo.git(&["commit", "--quiet", "-am", "Committed the popped change"]);

    let error = undo_stash(&cli, repo.path(), &record).unwrap_err();
    assert!(error.to_string().contains("repository changed"), "{error}");
}

/// Mixed reset on HEAD with conflicted files from a stash: the index is
/// cleared (files kept), undo brings the unmerged entries back (M9-04).
#[test]
fn mixed_reset_on_head_clears_conflicts_and_is_undone() {
    use git_core::{current_branch, record_index, reset, undo_history, HistoryChange, HistoryUndo};
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);
    apply_stash(&cli, &id, true).unwrap();
    let before = state(&repo);
    let head = repo.head();

    let (index_tree, unmerged) = record_index(&cli).unwrap();
    assert_eq!(state(&repo), before, "recording leaves the index alone");
    reset(&cli, &head, git_core::ResetMode::Mixed).unwrap();
    assert!(unmerged_paths(&cli).unwrap().is_empty());
    assert_eq!(files(repo.path()), before.files, "files are kept");

    let change = HistoryChange {
        label: "Mixed reset".into(),
        kind: HistoryUndo::Reset {
            mode: git_core::ResetMode::Mixed,
            index_tree,
            unmerged,
        },
        before: head.clone(),
        after: head,
        branch: current_branch(&cli).unwrap(),
    };
    undo_history(&cli, repo.path(), &change).unwrap();
    assert_eq!(state(&repo), before);
}

/// Hard reset on HEAD with conflicted files: everything uncommitted goes,
/// undo restores files and unmerged entries (M9-04).
#[test]
fn hard_reset_on_head_clears_conflicts_and_is_undone() {
    use git_core::{hard_reset_with_snapshot, undo_hard_reset};
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    let id = stash_then_conflicting_merge(&repo);
    apply_stash(&cli, &id, true).unwrap();
    repo.write("untracked.txt", "stays\n");
    let before = state(&repo);
    let head = repo.head();

    let snapshot = hard_reset_with_snapshot(&cli, repo.path(), &head).unwrap();
    assert!(unmerged_paths(&cli).unwrap().is_empty());
    assert_eq!(repo.git(&["status", "--porcelain"]), "?? untracked.txt");

    undo_hard_reset(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(state(&repo), before);
}
