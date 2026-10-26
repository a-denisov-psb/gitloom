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

fn rev(repo: &TestRepo, name: &str) -> String {
    repo.git(&["rev-parse", name])
}

#[test]
fn reword_head_amends_only_the_message() {
    let (repo, base) = history();
    repo.write("staged.txt", "staged\n");
    repo.git(&["add", "staged.txt"]);
    let parent = rev(&repo, "HEAD~1");
    git_core::reword_commit(&backend(&repo), &repo.head(), "four v2\n", false).unwrap();
    assert_eq!(messages(&repo, &base)[0], "four v2");
    assert_eq!(repo.parents("HEAD"), [parent]);
    assert_eq!(files(&repo, "HEAD"), "four.txt", "the index stays out");
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "staged.txt");
}

#[test]
fn reword_an_older_commit() {
    let (repo, base) = history();
    let two = rev(&repo, "HEAD~2");
    assert_eq!(
        git_core::reword_commit(&backend(&repo), &two, "two v2\n\nbody\n", false).unwrap(),
        MergeOutcome::Done
    );
    assert_eq!(
        messages(&repo, &base),
        ["four", "three", "two v2\n\nbody", "one"]
    );
}

#[test]
fn squash_selected_commits_keeps_later_ones() {
    let (repo, base) = history();
    let ids = vec![rev(&repo, "HEAD~1"), rev(&repo, "HEAD~2")];
    git_core::squash_commits(&backend(&repo), &ids, "two and three\n", false).unwrap();
    assert_eq!(messages(&repo, &base), ["four", "two and three", "one"]);
    assert_eq!(files(&repo, "HEAD~1"), "three.txt\ntwo.txt");
}

#[test]
fn squash_rejects_non_contiguous_selections() {
    let (repo, _) = history();
    let head = repo.head();
    let ids = vec![rev(&repo, "HEAD"), rev(&repo, "HEAD~2")];
    let error = git_core::squash_commits(&backend(&repo), &ids, "x\n", false).unwrap_err();
    assert!(error.to_string().contains("follow each other"), "{error}");
    assert_eq!(repo.head(), head);
    assert!(git_core::squash_commits(&backend(&repo), &ids[..1], "x\n", false).is_err());
}

#[test]
fn reword_rejects_commits_off_the_linear_history() {
    let (repo, _) = history();
    repo.git(&["switch", "--quiet", "-c", "side", "HEAD~1"]);
    let side = repo.commit("side.txt", "side\n", "side");
    repo.checkout("main");
    repo.merge("side", "merge side");
    let head = repo.head();
    // "side" is reachable through the merge, not on the first-parent line.
    let error = git_core::reword_commit(&backend(&repo), &side, "x\n", false).unwrap_err();
    assert!(error.to_string().contains("follow each other"), "{error}");
    assert_eq!(repo.head(), head);
}

#[test]
fn reword_refuses_when_a_merge_follows() {
    let (repo, _) = history();
    let two = rev(&repo, "HEAD~2");
    repo.git(&["switch", "--quiet", "-c", "side", "HEAD~1"]);
    repo.commit("side.txt", "side\n", "side");
    repo.checkout("main");
    repo.merge("side", "merge side");
    let head = repo.head();
    let error = git_core::reword_commit(&backend(&repo), &two, "x\n", false).unwrap_err();
    assert!(error.to_string().contains("without merges"), "{error}");
    assert_eq!(repo.head(), head);
}

#[test]
fn reword_and_squash_are_undone_and_redone() {
    for squash in [false, true] {
        let (repo, _) = history();
        let cli = backend(&repo);
        let before = repo.head();
        let branch = git_core::current_branch(&cli).unwrap();
        if squash {
            let ids = vec![rev(&repo, "HEAD"), rev(&repo, "HEAD~1")];
            git_core::squash_commits(&cli, &ids, "three and four\n", false).unwrap();
        } else {
            git_core::reword_commit(&cli, &rev(&repo, "HEAD~1"), "three v2\n", false).unwrap();
        }
        let entry = git_core::HistoryChange {
            label: "test".into(),
            kind: git_core::HistoryUndo::Keep,
            before: before.clone(),
            after: repo.head(),
            branch,
        };
        git_core::undo_history(&cli, repo.path(), &entry).unwrap();
        assert_eq!(repo.head(), before, "squash: {squash}");
        git_core::redo_history(&cli, repo.path(), &entry).unwrap();
        assert_eq!(repo.head(), entry.after, "squash: {squash}");
    }
}
