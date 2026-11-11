//! Discarding conflicted files and undoing it (M9-03, ADR-043).

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    discard_everything, discard_files, restore_snapshot, unmerged_paths, ChangeKind, CliBackend,
    FileChange, GitBackend, GixBackend,
};
use support::TestRepo;

fn isolated() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.commit("f.txt", "a\nb\n", "Base");
    repo.commit("gone.txt", "g\n", "Gone");
    repo
}

/// `f.txt` conflicts (both modified), `gone.txt` is deleted on main and
/// modified on topic (not in HEAD).
fn conflicted(repo: &TestRepo) {
    repo.git(&["switch", "--quiet", "-c", "topic"]);
    repo.write("f.txt", "a\nTopic\n");
    repo.write("gone.txt", "g topic\n");
    repo.git(&["commit", "--quiet", "-am", "Topic"]);
    repo.checkout("main");
    repo.write("f.txt", "a\nMain\n");
    repo.git(&["rm", "--quiet", "gone.txt"]);
    repo.git(&["commit", "--quiet", "-am", "Main"]);
    assert!(!repo.try_git(&["merge", "--quiet", "topic"]));
}

fn change(path: &str, kind: ChangeKind) -> FileChange {
    FileChange {
        path: path.into(),
        kind,
        old_path: None,
    }
}

fn read(repo: &TestRepo, path: &str) -> Option<Vec<u8>> {
    std::fs::read(repo.path().join(path)).ok()
}

#[test]
fn discard_conflicted_file_in_head_and_undo() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    conflicted(&repo);
    let index = repo.git(&["ls-files", "-s"]);
    let content = read(&repo, "f.txt");

    let snapshot = discard_files(
        &cli,
        repo.path(),
        &[change("f.txt", ChangeKind::Conflicted)],
    )
    .unwrap();
    assert_eq!(unmerged_paths(&cli).unwrap(), ["gone.txt"]);
    assert_eq!(read(&repo, "f.txt").unwrap(), b"a\nMain\n");
    assert_eq!(repo.git(&["diff", "HEAD", "--", "f.txt"]), "");

    let redo = restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(repo.git(&["ls-files", "-s"]), index);
    assert_eq!(read(&repo, "f.txt"), content);

    // Redo discards again.
    restore_snapshot(&cli, repo.path(), &redo).unwrap();
    assert_eq!(unmerged_paths(&cli).unwrap(), ["gone.txt"]);
    assert_eq!(read(&repo, "f.txt").unwrap(), b"a\nMain\n");
}

#[test]
fn discard_conflicted_file_not_in_head_and_undo() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    conflicted(&repo);
    let index = repo.git(&["ls-files", "-s"]);
    let content = read(&repo, "gone.txt");
    assert!(content.is_some());

    let snapshot = discard_files(
        &cli,
        repo.path(),
        &[change("gone.txt", ChangeKind::Conflicted)],
    )
    .unwrap();
    assert_eq!(unmerged_paths(&cli).unwrap(), ["f.txt"]);
    assert_eq!(repo.git(&["ls-files", "--", "gone.txt"]), "");
    assert_eq!(read(&repo, "gone.txt"), None);

    restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(repo.git(&["ls-files", "-s"]), index);
    assert_eq!(read(&repo, "gone.txt"), content);
}

#[test]
fn discard_everything_with_conflicted_and_normal_changes() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.commit("other.txt", "o\n", "Other");
    conflicted(&repo);
    repo.write("other.txt", "edited\n");
    repo.write("untracked.txt", "new\n");
    let index = repo.git(&["ls-files", "-s"]);
    let files: Vec<_> = ["f.txt", "gone.txt", "other.txt", "untracked.txt"]
        .iter()
        .map(|p| read(&repo, p))
        .collect();

    let tree = GixBackend::discover(repo.path())
        .unwrap()
        .working_tree()
        .unwrap();
    assert!(tree
        .unstaged
        .iter()
        .any(|f| f.kind == ChangeKind::Conflicted));
    let snapshot = discard_everything(&cli, repo.path(), &tree).unwrap();
    assert!(unmerged_paths(&cli).unwrap().is_empty());
    assert_eq!(repo.git(&["status", "--porcelain"]), "");

    restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(repo.git(&["ls-files", "-s"]), index);
    let restored: Vec<_> = ["f.txt", "gone.txt", "other.txt", "untracked.txt"]
        .iter()
        .map(|p| read(&repo, p))
        .collect();
    assert_eq!(restored, files);
}
