//! Gitloom as Git editor (ADR-021): the real binary answers Git's sequence
//! and message editor calls with prepared content.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../../git-core/tests/support/mod.rs"]
mod support;

use git_core::{CliBackend, EditorInput, Error, GitWriter, RunOptions};
use support::TestRepo;

/// Repository with commits `one`, `two`, `three` on `main`; returns the
/// repository and the id of `one`.
fn three_commits() -> (TestRepo, String) {
    let repo = TestRepo::new();
    // `git rebase` run by the backend commits with the repository identity.
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    let one = repo.commit("one.txt", "one\n", "one");
    repo.commit("two.txt", "two\n", "two");
    repo.commit("three.txt", "three\n", "three");
    (repo, one)
}

fn backend(repo: &TestRepo) -> CliBackend {
    CliBackend::new(repo.path()).with_editor(env!("CARGO_BIN_EXE_gitloom"))
}

/// Short id of `rev`.
fn short(repo: &TestRepo, rev: &str) -> String {
    repo.git(&["rev-parse", "--short", rev])
}

/// Commit subjects from HEAD down to (excluding) `base`.
fn subjects(repo: &TestRepo, base: &str) -> Vec<String> {
    repo.git(&["log", "--format=%B%x00", &format!("{base}..HEAD")])
        .split('\0')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn rebase_in_progress(repo: &TestRepo) -> bool {
    repo.path().join(".git/rebase-merge").exists()
}

#[test]
fn prepared_todo_list_reorders_commits() {
    let (repo, one) = three_commits();
    let todo = format!(
        "pick {} three\npick {} two\n",
        short(&repo, "HEAD"),
        short(&repo, "HEAD~1")
    );
    let input = EditorInput {
        sequence: Some(todo),
        messages: Vec::new(),
    };
    backend(&repo)
        .run_editing(&["rebase", "-i", &one], &input, RunOptions::default())
        .unwrap();
    assert_eq!(subjects(&repo, &one), ["two", "three"]);
    assert!(!rebase_in_progress(&repo));
}

#[test]
fn prepared_messages_answer_reword_and_squash_in_order() {
    let (repo, one) = three_commits();
    let todo = format!(
        "reword {} two\nsquash {} three\n",
        short(&repo, "HEAD~1"),
        short(&repo, "HEAD")
    );
    let input = EditorInput {
        sequence: Some(todo),
        messages: vec!["two reworded\n".into(), "two and three\n\nbody\n".into()],
    };
    backend(&repo)
        .run_editing(&["rebase", "-i", &one], &input, RunOptions::default())
        .unwrap();
    assert_eq!(subjects(&repo, &one), ["two and three\n\nbody"]);
    assert_eq!(
        repo.git(&["show", "--format=", "--name-only", "HEAD"]),
        "three.txt\ntwo.txt"
    );
}

#[test]
fn prepared_message_is_used_for_a_commit() {
    let (repo, _) = three_commits();
    repo.write("four.txt", "four\n");
    repo.git(&["add", "four.txt"]);
    let input = EditorInput {
        sequence: None,
        messages: vec!["four\n".into()],
    };
    backend(&repo)
        .run_editing(&["commit"], &input, RunOptions::default())
        .unwrap();
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "four");
}

#[test]
fn missing_todo_list_aborts_the_rebase() {
    let (repo, one) = three_commits();
    let head = repo.head();
    let result = backend(&repo).run_editing(
        &["rebase", "-i", &one],
        &EditorInput::default(),
        RunOptions::default(),
    );
    match result {
        Err(Error::Command { stderr, .. }) => {
            assert!(stderr.contains("no prepared todo list"), "{stderr}")
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(repo.head(), head);
    assert!(!rebase_in_progress(&repo));
}

#[test]
fn missing_message_stops_without_hanging() {
    let (repo, one) = three_commits();
    let head = repo.head();
    let input = EditorInput {
        sequence: Some(format!("reword {} two\n", short(&repo, "HEAD~1"))),
        messages: Vec::new(),
    };
    assert!(backend(&repo)
        .run_editing(&["rebase", "-i", &one], &input, RunOptions::default())
        .is_err());
    // Git keeps the rebase so it can be continued or aborted (M4-03).
    assert!(rebase_in_progress(&repo));
    repo.git(&["rebase", "--abort"]);
    assert_eq!(repo.head(), head);
}

#[test]
fn without_editor_program_run_editing_fails() {
    let (repo, one) = three_commits();
    let result = CliBackend::new(repo.path()).run_editing(
        &["rebase", "-i", &one],
        &EditorInput::default(),
        RunOptions::default(),
    );
    assert!(matches!(result, Err(Error::Backend(_))), "{result:?}");
    assert!(!rebase_in_progress(&repo));
}
