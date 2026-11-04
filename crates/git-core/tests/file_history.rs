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

/// `git blame` output reduced to (short commit, line) pairs for comparison.
fn cli_blame(repo: &TestRepo, rev: Option<&str>, path: &str) -> Vec<(String, String)> {
    let mut args = vec!["blame", "-s", "-l"];
    args.extend(rev);
    args.extend(["--", path]);
    repo.git(&args)
        .lines()
        .map(|line| {
            let (id, rest) = line.split_once(' ').unwrap();
            let content = rest.split_once(") ").map_or("", |(_, c)| c);
            (id.trim_start_matches('^').to_owned(), content.to_owned())
        })
        .collect()
}

#[test]
fn blame_matches_git_blame() {
    use git_core::{blame, UNCOMMITTED};
    let repo = TestRepo::new();
    repo.commit("f.txt", "one\ntwo\nthree\n", "first");
    repo.commit("f.txt", "one\nTWO\nthree\nfour\n", "second");
    let cli = CliBackend::new(repo.path());
    let ours = blame(&cli, "f.txt", Some("HEAD"), &CancelToken::new()).unwrap();
    let pairs: Vec<(String, String)> = ours
        .lines
        .iter()
        .map(|(c, l)| (ours.commits[*c].id.clone(), l.clone()))
        .collect();
    let theirs: Vec<(String, String)> = cli_blame(&repo, Some("HEAD"), "f.txt")
        .into_iter()
        .map(|(id, l)| {
            let full = pairs
                .iter()
                .find(|(p, _)| p.starts_with(&id))
                .unwrap()
                .0
                .clone();
            (full, l)
        })
        .collect();
    assert_eq!(pairs, theirs);
    assert_eq!(ours.groups.len(), 4, "first, second, first, second");
    assert_eq!(ours.commits.iter().filter(|c| c.boundary).count(), 1);

    // Working copy: changed lines are not committed yet.
    repo.write("f.txt", "one\nTWO\nthree\nfour\nfive\n");
    let wip = blame(&cli, "f.txt", None, &CancelToken::new()).unwrap();
    let last = wip.lines.last().unwrap();
    assert_eq!(last.1, "five");
    assert_eq!(wip.commits[last.0].id, UNCOMMITTED);
}

#[test]
fn blame_respects_the_mailmap() {
    use git_core::blame;
    let repo = TestRepo::new();
    repo.commit("f.txt", "one\n", "first");
    let author = repo.git(&["log", "-1", "--format=%an <%ae>"]);
    repo.commit(
        ".mailmap",
        &format!("Mapped Name <mapped@example.com> {author}\n"),
        "mailmap",
    );
    let cli = CliBackend::new(repo.path());
    let ours = blame(&cli, "f.txt", Some("HEAD"), &CancelToken::new()).unwrap();
    assert_eq!(ours.commits[0].author, "Mapped Name");
}
