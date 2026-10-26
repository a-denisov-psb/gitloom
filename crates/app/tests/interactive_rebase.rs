//! Interactive rebase with a prepared plan (M4-07): the real binary answers
//! Git's sequence and message editors (ADR-021).

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../../git-core/tests/support/mod.rs"]
mod support;

use git_core::{
    build_todo, default_message, rebase_commits, rebase_interactive, CliBackend, GitBackend as _,
    GitWriter as _, GixBackend, MergeOutcome, RepoState, TodoAction, TodoEntry,
};
use support::TestRepo;
use TodoAction::*;

/// `base` followed by commits "one" … "four", each adding its own file.
fn history() -> (TestRepo, String) {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    let base = repo.commit("base.txt", "base\n", "base");
    for name in ["one", "two", "three", "four"] {
        repo.commit(&format!("{name}.txt"), &format!("{name}\n"), name);
    }
    (repo, base)
}

fn backend(repo: &TestRepo) -> CliBackend {
    CliBackend::new(repo.path()).with_editor(env!("CARGO_BIN_EXE_gitloom"))
}

/// The plan as loaded for the panel, with `actions` applied in todo order.
fn plan(repo: &TestRepo, base: &str, actions: &[TodoAction]) -> Vec<TodoEntry> {
    let commits = rebase_commits(&backend(repo), base).unwrap();
    assert_eq!(commits.len(), actions.len());
    commits
        .into_iter()
        .zip(actions)
        .map(|((id, message), &action)| TodoEntry {
            id,
            action,
            message,
        })
        .collect()
}

fn run(
    repo: &TestRepo,
    base: &str,
    entries: &[TodoEntry],
    messages: &[(usize, &str)],
) -> MergeOutcome {
    let input = build_todo(entries, |head| {
        messages
            .iter()
            .find(|(i, _)| *i == head)
            .map_or_else(|| default_message(entries, head), |(_, m)| (*m).to_owned())
    })
    .unwrap();
    rebase_interactive(&backend(repo), base, &input, false).unwrap()
}

/// Full messages from HEAD down to `base` (exclusive), newest first.
fn messages(repo: &TestRepo, base: &str) -> Vec<String> {
    repo.git(&["log", "--format=%B%x00", &format!("{base}..HEAD")])
        .split('\0')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn files(repo: &TestRepo, rev: &str) -> String {
    repo.git(&["show", "--format=", "--name-only", rev])
}

fn state(repo: &TestRepo) -> RepoState {
    GixBackend::discover(repo.path()).unwrap().state().unwrap()
}

#[test]
fn loads_the_commits_oldest_first() {
    let (repo, base) = history();
    let commits = rebase_commits(&backend(&repo), &base).unwrap();
    let subjects: Vec<_> = commits.iter().map(|(_, m)| m.trim()).collect();
    assert_eq!(subjects, ["one", "two", "three", "four"]);
}

#[test]
fn reorder() {
    let (repo, base) = history();
    let mut entries = plan(&repo, &base, &[Pick, Pick, Pick, Pick]);
    entries.swap(0, 3);
    assert_eq!(run(&repo, &base, &entries, &[]), MergeOutcome::Done);
    assert_eq!(messages(&repo, &base), ["one", "three", "two", "four"]);
    assert_eq!(state(&repo), RepoState::Clean);
}

#[test]
fn reword() {
    let (repo, base) = history();
    let entries = plan(&repo, &base, &[Pick, Reword, Pick, Reword]);
    run(
        &repo,
        &base,
        &entries,
        &[(1, "two v2\n\nbody"), (3, "four v2")],
    );
    assert_eq!(
        messages(&repo, &base),
        ["four v2", "three", "two v2\n\nbody", "one"]
    );
}

#[test]
fn squash_combines_messages() {
    let (repo, base) = history();
    let entries = plan(&repo, &base, &[Pick, Squash, Squash, Pick]);
    assert_eq!(default_message(&entries, 0), "one\n\ntwo\n\nthree");
    run(&repo, &base, &entries, &[]);
    assert_eq!(messages(&repo, &base), ["four", "one\n\ntwo\n\nthree"]);
    assert_eq!(files(&repo, "HEAD~1"), "one.txt\nthree.txt\ntwo.txt");
}

#[test]
fn fixup_keeps_the_first_message() {
    let (repo, base) = history();
    let entries = plan(&repo, &base, &[Pick, Fixup, Pick, Fixup]);
    run(&repo, &base, &entries, &[]);
    assert_eq!(messages(&repo, &base), ["three", "one"]);
    assert_eq!(files(&repo, "HEAD"), "four.txt\nthree.txt");
}

#[test]
fn drop_removes_commits() {
    let (repo, base) = history();
    let entries = plan(&repo, &base, &[Drop, Pick, Drop, Pick]);
    run(&repo, &base, &entries, &[]);
    assert_eq!(messages(&repo, &base), ["four", "two"]);
    assert!(!repo.path().join("one.txt").exists());
}

#[test]
fn mixed_plan_hands_out_messages_in_order() {
    let (repo, base) = history();
    // Reword head of a squash chain (asked once, combined), then a reword.
    let entries = plan(&repo, &base, &[Reword, Squash, Fixup, Reword]);
    run(
        &repo,
        &base,
        &entries,
        &[(0, "one to three"), (3, "four reworded")],
    );
    assert_eq!(messages(&repo, &base), ["four reworded", "one to three"]);
}

#[test]
fn conflict_during_reorder_stops_the_rebase() {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    let base = repo.commit("a.txt", "1\n", "base");
    repo.commit("a.txt", "2\n", "two");
    repo.commit("a.txt", "3\n", "three");
    let head = repo.head();
    let mut entries = plan(&repo, &base, &[Pick, Pick]);
    entries.swap(0, 1);
    assert_eq!(run(&repo, &base, &entries, &[]), MergeOutcome::Conflicts);
    assert!(matches!(state(&repo), RepoState::Rebasing { .. }));
    backend(&repo)
        .abort_operation(git_core::Operation::Rebase)
        .unwrap();
    assert_eq!(repo.head(), head);
}
