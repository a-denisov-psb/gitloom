// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    discard_files, head_commit, move_head, restore_snapshot, ChangeKind, CliBackend, FileChange,
    GitWriter,
};
use support::TestRepo;

fn isolated() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo
}

/// Commits through CliBackend and returns (before, after) HEAD.
fn commit(repo: &TestRepo, message: &str, amend: bool) -> (Option<String>, String) {
    let cli = CliBackend::new(repo.path());
    let before = head_commit(&cli).unwrap();
    cli.commit(message, amend).unwrap();
    (before, head_commit(&cli).unwrap().unwrap())
}

#[test]
fn undo_and_redo_a_commit() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.write("a.txt", "b\n");
    repo.git(&["add", "a.txt"]);
    let (before, after) = commit(&repo, "Change", false);
    let cli = CliBackend::new(repo.path());

    move_head(&cli, Some(&after), before.as_deref(), "undo").unwrap();
    assert_eq!(head_commit(&cli).unwrap(), before);
    // Like reset --soft: the change is staged again.
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "a.txt");
    assert_eq!(repo.git(&["branch", "--show-current"]), "main");

    move_head(&cli, before.as_deref(), Some(&after), "redo").unwrap();
    assert_eq!(head_commit(&cli).unwrap().as_deref(), Some(after.as_str()));
    assert!(repo.git(&["diff", "--cached", "--name-only"]).is_empty());
}

#[test]
fn undo_and_redo_the_root_commit() {
    let repo = isolated();
    repo.write("first.txt", "1\n");
    repo.git(&["add", "first.txt"]);
    let (before, after) = commit(&repo, "Initial", false);
    assert!(before.is_none());
    let cli = CliBackend::new(repo.path());

    move_head(&cli, Some(&after), None, "undo").unwrap();
    assert!(head_commit(&cli).unwrap().is_none());
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "first.txt");

    move_head(&cli, None, Some(&after), "redo").unwrap();
    assert_eq!(repo.git(&["log", "--format=%s"]), "Initial");
    assert_eq!(repo.git(&["branch", "--show-current"]), "main");
}

#[test]
fn undo_an_amend() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.commit("b.txt", "b\n", "Original");
    let (before, after) = commit(&repo, "Amended", true);
    let cli = CliBackend::new(repo.path());
    move_head(&cli, Some(&after), before.as_deref(), "undo").unwrap();
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Original");
}

#[test]
fn undo_fails_when_head_moved() {
    let repo = isolated();
    repo.commit("a.txt", "a\n", "Base");
    repo.write("a.txt", "b\n");
    repo.git(&["add", "a.txt"]);
    let (before, after) = commit(&repo, "Change", false);
    repo.commit("c.txt", "c\n", "Someone else's commit");
    let cli = CliBackend::new(repo.path());
    let error = move_head(&cli, Some(&after), before.as_deref(), "undo").unwrap_err();
    assert!(
        error.to_string().contains("the repository changed"),
        "{error}"
    );
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]),
        "Someone else's commit"
    );
}

#[test]
fn undo_and_redo_a_discard() {
    let repo = isolated();
    repo.commit("tracked.txt", "original\n", "Base");
    repo.write("tracked.txt", "edited\n");
    repo.write("untracked.txt", "new\n");
    let cli = CliBackend::new(repo.path());
    let snapshot = discard_files(
        &cli,
        repo.path(),
        &[
            FileChange {
                path: "tracked.txt".into(),
                kind: ChangeKind::Modified,
                old_path: None,
            },
            FileChange {
                path: "untracked.txt".into(),
                kind: ChangeKind::Untracked,
                old_path: None,
            },
        ],
    )
    .unwrap();
    assert!(!repo.path().join("untracked.txt").exists());

    // Undo brings both files back and returns the state for redo.
    let redo = restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("tracked.txt")).unwrap(),
        "edited\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
        "new\n"
    );

    // Redo discards again.
    restore_snapshot(&cli, repo.path(), &redo).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("tracked.txt")).unwrap(),
        "original\n"
    );
    assert!(!repo.path().join("untracked.txt").exists());
}

#[cfg(unix)]
#[test]
fn restore_keeps_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt as _;
    let repo = isolated();
    repo.commit("run.sh", "echo 1\n", "Base");
    let path = repo.path().join("run.sh");
    std::fs::write(&path, "echo 2\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cli = CliBackend::new(repo.path());
    let snapshot = discard_files(
        &cli,
        repo.path(),
        &[FileChange {
            path: "run.sh".into(),
            kind: ChangeKind::Modified,
            old_path: None,
        }],
    )
    .unwrap();
    restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "echo 2\n");
}
