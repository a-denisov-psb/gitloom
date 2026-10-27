// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{file_history, tracked_files, CancelToken, CliBackend, Error};
use support::TestRepo;

fn summaries(repo: &TestRepo, path: &str) -> Vec<(String, String, Option<String>)> {
    let cli = CliBackend::new(repo.path());
    file_history(&cli, path, "HEAD", &CancelToken::new())
        .unwrap()
        .into_iter()
        .map(|e| (e.summary, e.path, e.old_path))
        .collect()
}

#[test]
fn linear_history_newest_first() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "1\n", "add a");
    repo.commit("b.txt", "x\n", "other file");
    repo.commit("a.txt", "2\n", "edit a");
    let history = summaries(&repo, "a.txt");
    assert_eq!(
        history,
        [
            ("edit a".into(), "a.txt".into(), None),
            ("add a".into(), "a.txt".into(), None),
        ]
    );
}

#[test]
fn renames_are_followed() {
    let repo = TestRepo::new();
    repo.commit("old.txt", "line 1\nline 2\nline 3\n", "add");
    repo.git(&["mv", "old.txt", "new.txt"]);
    repo.git(&["commit", "-q", "-m", "rename"]);
    repo.commit("new.txt", "line 1\nline 2\nline 3\nline 4\n", "edit");
    let history = summaries(&repo, "new.txt");
    assert_eq!(history.len(), 3);
    assert_eq!(history[0], ("edit".into(), "new.txt".into(), None));
    assert_eq!(
        history[1],
        ("rename".into(), "new.txt".into(), Some("old.txt".into()))
    );
    assert_eq!(history[2], ("add".into(), "old.txt".into(), None));
}

#[test]
fn deleted_and_added_again() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "1\n", "add");
    repo.git(&["rm", "-q", "a.txt"]);
    repo.git(&["commit", "-q", "-m", "delete"]);
    repo.commit("a.txt", "2\n", "add again");
    let cli = CliBackend::new(repo.path());
    let history = file_history(&cli, "a.txt", "HEAD", &CancelToken::new()).unwrap();
    let statuses: Vec<(&str, &str)> = history
        .iter()
        .map(|e| (e.summary.as_str(), e.status.as_str()))
        .collect();
    assert_eq!(
        statuses,
        [("add again", "A"), ("delete", "D"), ("add", "A")]
    );
}

#[test]
fn merged_branch_commits_appear() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "1\n", "base");
    repo.branch("side");
    repo.checkout("side");
    repo.commit("a.txt", "1\nside\n", "side change");
    repo.checkout("main");
    repo.commit("b.txt", "b\n", "main other");
    repo.merge("side", "merge side");
    // The merge itself did not change a.txt (taken from side as is).
    let history = summaries(&repo, "a.txt");
    let names: Vec<&str> = history.iter().map(|(s, _, _)| s.as_str()).collect();
    assert_eq!(names, ["side change", "base"]);
    assert_eq!(repo.parents("HEAD").len(), 2);
}

#[test]
fn paths_with_spaces_and_non_ascii() {
    let repo = TestRepo::new();
    repo.commit("dir name/Grüße ü.txt", "1\n", "add");
    repo.commit("dir name/Grüße ü.txt", "2\n", "edit");
    let history = summaries(&repo, "dir name/Grüße ü.txt");
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|(_, p, _)| p == "dir name/Grüße ü.txt"));

    let cli = CliBackend::new(repo.path());
    let files = tracked_files(&cli, "HEAD", &CancelToken::new()).unwrap();
    assert!(files.contains(&"dir name/Grüße ü.txt".to_owned()));
}

#[test]
fn cancelled_request_fails_with_cancelled() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "1\n", "add");
    let cli = CliBackend::new(repo.path());
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = file_history(&cli, "a.txt", "HEAD", &cancel);
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
}

#[test]
fn unknown_path_has_no_history() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "1\n", "add");
    assert!(summaries(&repo, "missing.txt").is_empty());
}
