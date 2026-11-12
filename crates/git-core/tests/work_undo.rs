//! Undo and redo of stage/unstage, mark resolved, stash push and stash
//! drop (M12-07): index (`git ls-files -s`), working tree and stash list
//! before and after, and refusals after outside changes.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::work_undo::{
    record_work_change, redo_stash_drop, redo_stash_push, stash_drop_recorded, stash_push_recorded,
    switch_work_change, undo_stash_drop, undo_stash_push, Scope,
};
use git_core::CliBackend;
use support::TestRepo;

fn repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    repo.commit("a.txt", "a\n", "base");
    repo.commit("b.txt", "b\n", "b");
    repo
}

/// Index, working tree status and stash list in one comparable value.
fn state(repo: &TestRepo) -> (String, String, String) {
    (
        repo.git(&["ls-files", "-s"]),
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        repo.git(&["stash", "list", "--format=%H %gs"]),
    )
}

#[test]
fn stage_and_unstage_paths_undo_and_redo() {
    let repo = repo();
    let git = CliBackend::new(repo.path());
    repo.write("a.txt", "changed\n");
    repo.write("new.txt", "new\n");
    let before = state(&repo);
    let paths = vec!["a.txt".to_owned(), "new.txt".to_owned()];
    let change = record_work_change(
        &git,
        repo.path(),
        "Stage",
        Scope::Paths(paths.clone()),
        &[],
        |g| g.stage(&paths),
    )
    .unwrap()
    .unwrap();
    let after = state(&repo);
    assert_ne!(before, after);
    switch_work_change(&git, repo.path(), &change, true).unwrap();
    assert_eq!(state(&repo), before);
    switch_work_change(&git, repo.path(), &change, false).unwrap();
    assert_eq!(state(&repo), after);
    // Outside change to an affected path: undo refuses and changes nothing.
    repo.git(&["reset", "--quiet", "--", "new.txt"]);
    let outside = state(&repo);
    let error = switch_work_change(&git, repo.path(), &change, true).unwrap_err();
    assert!(error.to_string().contains("Cannot undo"), "{error}");
    assert_eq!(state(&repo), outside);
    // Nothing changed: no entry.
    assert!(record_work_change(
        &git,
        repo.path(),
        "Stage",
        Scope::Paths(vec!["b.txt".into()]),
        &[],
        |g| { g.stage(&["b.txt".to_owned()]) }
    )
    .unwrap()
    .is_none());
}

#[test]
fn unstage_all_undo_restores_the_whole_index() {
    let repo = repo();
    let git = CliBackend::new(repo.path());
    repo.write("a.txt", "staged\n");
    repo.write("c.txt", "c\n");
    repo.git(&["add", "-A"]);
    let before = state(&repo);
    let change = record_work_change(&git, repo.path(), "Unstage all", Scope::Whole, &[], |g| {
        g.unstage_all()
    })
    .unwrap()
    .unwrap();
    let after = state(&repo);
    switch_work_change(&git, repo.path(), &change, true).unwrap();
    assert_eq!(state(&repo), before);
    switch_work_change(&git, repo.path(), &change, false).unwrap();
    assert_eq!(state(&repo), after);
}

#[test]
fn mark_resolved_restores_conflict_stages_and_content() {
    let repo = repo();
    let git = CliBackend::new(repo.path());
    repo.branch("topic");
    repo.commit("a.txt", "main\n", "main");
    repo.checkout("topic");
    repo.commit("a.txt", "topic\n", "topic");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "topic"]));
    let before = state(&repo);
    let conflicted = std::fs::read(repo.path().join("a.txt")).unwrap();
    let paths = vec!["a.txt".to_owned()];
    let change = record_work_change(
        &git,
        repo.path(),
        "Resolve a.txt",
        Scope::Paths(paths.clone()),
        &paths,
        |g| git_core::resolve_conflict(g, "a.txt", b"resolved\n"),
    )
    .unwrap()
    .unwrap();
    let after = state(&repo);
    assert!(before.0.contains(" 1\ta.txt") && !after.0.contains(" 1\ta.txt"));
    switch_work_change(&git, repo.path(), &change, true).unwrap();
    assert_eq!(state(&repo), before);
    assert_eq!(
        std::fs::read(repo.path().join("a.txt")).unwrap(),
        conflicted
    );
    switch_work_change(&git, repo.path(), &change, false).unwrap();
    assert_eq!(state(&repo), after);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "resolved\n"
    );
    // Edited after resolving: undo refuses.
    repo.write("a.txt", "edited\n");
    assert!(switch_work_change(&git, repo.path(), &change, true).is_err());
}

#[test]
fn stash_push_undo_and_redo() {
    let repo = repo();
    let git = CliBackend::new(repo.path());
    repo.write("a.txt", "staged\n");
    repo.git(&["add", "a.txt"]);
    repo.write("b.txt", "unstaged\n");
    repo.write("new.txt", "untracked\n");
    let before = state(&repo);
    let record = stash_push_recorded(&git, Some("my work")).unwrap();
    assert_eq!(
        repo.git(&["stash", "list", "--format=%gs"]),
        "On main: my work"
    );
    undo_stash_push(&git, &record).unwrap();
    assert_eq!(state(&repo), before, "staged stays staged (--index)");
    let record = redo_stash_push(&git, &record).unwrap();
    assert_eq!(
        repo.git(&["stash", "list", "--format=%gs"]),
        "On main: my work"
    );
    // A change after the push: undo refuses, the stash stays.
    repo.write("other.txt", "x\n");
    let outside = state(&repo);
    assert!(undo_stash_push(&git, &record).is_err());
    assert_eq!(state(&repo), outside);
}

#[test]
fn stash_drop_undo_keeps_the_position() {
    let repo = repo();
    let git = CliBackend::new(repo.path());
    for name in ["one", "two", "three"] {
        repo.write("a.txt", &format!("{name}\n"));
        repo.git(&["stash", "push", "--quiet", "-m", name]);
    }
    let before = state(&repo);
    let middle = repo.git(&["rev-parse", "stash@{1}"]);
    let record = stash_drop_recorded(&git, &middle).unwrap();
    assert_eq!(record.position, 1);
    let after = state(&repo);
    undo_stash_drop(&git, &record).unwrap();
    assert_eq!(state(&repo), before);
    redo_stash_drop(&git, &record).unwrap();
    assert_eq!(state(&repo), after);
}

#[test]
fn rename_stash_in_place_with_undo_and_redo() {
    use git_core::work_undo::{rename_stash, renamed_message, switch_stash_rename};
    let repo = repo();
    let git = CliBackend::new(repo.path());
    for name in ["one", "two", "three"] {
        repo.write("a.txt", &format!("{name}\n"));
        repo.git(&["stash", "push", "--quiet", "-m", name]);
    }
    let middle = repo.git(&["rev-parse", "stash@{1}"]);
    let list = || repo.git(&["stash", "list", "--format=%H %gs"]);
    let before = list();
    let rename = rename_stash(&git, &middle, "work/better name").unwrap();
    assert_eq!(
        repo.git(&["stash", "list", "--format=%gs"]),
        "On main: three\nOn main: work/better name\nOn main: one"
    );
    let renamed = repo.git(&["rev-parse", "stash@{1}"]);
    assert_ne!(renamed, middle);
    // Same tree, parents and author as the old stash commit.
    let facts = |rev: &str| repo.git(&["log", "-1", "--format=%T %P %an %ae %ad", rev]);
    assert_eq!(facts(&renamed), facts(&middle));
    assert_eq!(
        repo.git(&["stash", "show", "-p", "stash@{1}"]),
        repo.git(&["diff", &format!("{middle}^1"), &middle])
    );
    let after = list();
    switch_stash_rename(&git, &rename, true).unwrap();
    assert_eq!(list(), before);
    switch_stash_rename(&git, &rename, false).unwrap();
    assert_eq!(list(), after);
    // A new stash on top: undo refuses and changes nothing.
    repo.write("a.txt", "four\n");
    repo.git(&["stash", "push", "--quiet", "-m", "four"]);
    let outside = list();
    assert!(switch_stash_rename(&git, &rename, true).is_err());
    assert_eq!(list(), outside);

    assert_eq!(
        renamed_message("WIP on feature/x: abc123 Fix", "n"),
        "On feature/x: n"
    );
    assert_eq!(renamed_message("On main: old", "n"), "On main: n");
    assert_eq!(renamed_message("custom", "n"), "n");
}
