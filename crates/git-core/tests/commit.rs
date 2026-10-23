// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{CliBackend, GitBackend, GitWriter, GixBackend};
use support::TestRepo;

/// Repository whose commits through CliBackend do not depend on the user's
/// global Git configuration (identity, signing, hooks).
fn isolated() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.git(&["config", "core.hooksPath", ".git/hooks"]);
    repo
}

fn writer(repo: &TestRepo) -> CliBackend {
    CliBackend::new(repo.path())
}

#[test]
fn commits_the_index_with_the_message() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.write("a.txt", "b\n");
    repo.git(&["add", "a.txt"]);
    writer(&repo)
        .commit("Summary line\n\nBody text.", false)
        .unwrap();
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s|%b"]),
        "Summary line|Body text."
    );
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "2");
}

#[test]
fn first_commit_in_an_empty_repository() {
    let repo = isolated();
    repo.write("first.txt", "1\n");
    repo.git(&["add", "first.txt"]);
    writer(&repo).commit("Initial commit", false).unwrap();
    assert_eq!(repo.git(&["log", "--format=%s"]), "Initial commit");
}

#[cfg(unix)]
#[test]
fn failing_pre_commit_hook_reports_its_output() {
    use git_core::Error;
    use std::os::unix::fs::PermissionsExt as _;
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    let hook = repo.path().join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(
        &hook,
        "#!/bin/sh\necho 'lint failed: trailing spaces' >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    repo.write("a.txt", "b\n");
    repo.git(&["add", "a.txt"]);

    let error = writer(&repo).commit("Blocked", false).unwrap_err();
    match error {
        Error::Command { stderr, .. } => assert!(stderr.contains("lint failed"), "{stderr}"),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "1");
}

#[test]
fn commit_template_is_read() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    let template = repo.path().join(".git/template.txt");
    std::fs::write(&template, "Ticket: \n\n# Describe the change\n").unwrap();
    repo.git(&["config", "commit.template", template.to_str().unwrap()]);
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(
        backend.commit_template().unwrap().as_deref(),
        Some("Ticket: \n\n# Describe the change\n")
    );
}

#[test]
fn no_template_configured() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["config", "commit.template", ""]);
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert!(backend.commit_template().unwrap().is_none());
}

#[test]
fn commit_concludes_a_merge() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.branch("other");
    repo.commit("a.txt", "main\n", "Main");
    repo.checkout("other");
    repo.commit("a.txt", "other\n", "Other");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "--quiet", "other"]));

    let backend = GixBackend::discover(repo.path()).unwrap();
    let message = backend.merge_message().unwrap().expect("merge in progress");
    assert!(message.starts_with("Merge branch 'other'"), "{message}");

    repo.write("a.txt", "resolved\n");
    repo.git(&["add", "a.txt"]);
    writer(&repo).commit(&message, false).unwrap();
    assert_eq!(
        repo.git(&["rev-list", "--parents", "-n", "1", "HEAD"])
            .split(' ')
            .count(),
        3
    );
    assert!(backend.merge_message().unwrap().is_none());
}

#[test]
fn amend_message_only() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.commit("b.txt", "b\n", "Typo in mesage");
    let parent = repo.git(&["rev-parse", "HEAD~1"]);
    writer(&repo).commit("Fixed message", true).unwrap();
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Fixed message");
    assert_eq!(repo.git(&["rev-parse", "HEAD~1"]), parent);
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "2");
    assert_eq!(
        repo.git(&["show", "--format=", "--name-only", "HEAD"]),
        "b.txt"
    );
}

#[test]
fn amend_with_staged_changes() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.commit("b.txt", "b\n", "Add b");
    repo.write("c.txt", "c\n");
    repo.git(&["add", "c.txt"]);
    writer(&repo).commit("Add b and c", true).unwrap();
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "2");
    assert_eq!(
        repo.git(&["show", "--format=", "--name-only", "HEAD"]),
        "b.txt\nc.txt"
    );
}
