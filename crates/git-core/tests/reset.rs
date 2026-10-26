// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{hard_reset_with_snapshot, reset, undo_hard_reset, CliBackend, ResetMode};
use support::TestRepo;

/// base (a.txt) → second (b.txt) → third (a.txt changed); returns the ids.
fn history() -> (TestRepo, String, String) {
    let repo = TestRepo::new();
    let base = repo.commit("a.txt", "base\n", "base");
    repo.commit("b.txt", "b\n", "second");
    let third = repo.commit("a.txt", "third\n", "third");
    (repo, base, third)
}

fn read(repo: &TestRepo, path: &str) -> Option<Vec<u8>> {
    std::fs::read(repo.path().join(path)).ok()
}

fn status(repo: &TestRepo) -> String {
    repo.git(&["status", "--porcelain", "--untracked-files=all"])
}

#[test]
fn soft_reset_keeps_the_changes_staged() {
    let (repo, base, _) = history();
    reset(&CliBackend::new(repo.path()), &base, ResetMode::Soft).unwrap();
    assert_eq!(repo.head(), base);
    assert_eq!(status(&repo), "M  a.txt\nA  b.txt");
}

#[test]
fn mixed_reset_keeps_the_changes_unstaged() {
    let (repo, base, _) = history();
    reset(&CliBackend::new(repo.path()), &base, ResetMode::Mixed).unwrap();
    assert_eq!(repo.head(), base);
    // `git` output is trimmed: " M a.txt" loses its leading space.
    assert_eq!(status(&repo), "M a.txt\n?? b.txt");
}

#[test]
fn hard_reset_discards_changes_but_keeps_untracked_files() {
    let (repo, base, _) = history();
    repo.write("a.txt", "local\n");
    repo.write("new.txt", "untracked\n");
    reset(&CliBackend::new(repo.path()), &base, ResetMode::Hard).unwrap();
    assert_eq!(repo.head(), base);
    assert_eq!(read(&repo, "a.txt").unwrap(), b"base\n");
    assert!(read(&repo, "b.txt").is_none());
    assert_eq!(status(&repo), "?? new.txt");
}

#[test]
fn hard_reset_snapshot_restores_head_index_and_files() {
    let (repo, base, third) = history();
    // Staged and unstaged changes, CRLF content, an executable bit, and an
    // untracked file that the reset target overwrites.
    repo.write("a.txt", "staged\r\n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "unstaged\r\n");
    repo.write("c.txt", "kept\n");
    let before_status = status(&repo);
    let cli = CliBackend::new(repo.path());

    let snapshot = hard_reset_with_snapshot(&cli, repo.path(), &base).unwrap();
    assert_eq!(snapshot.head_before, third);
    assert_eq!(snapshot.head_after, base);
    assert_eq!(repo.head(), base);
    assert_eq!(read(&repo, "a.txt").unwrap(), b"base\n");
    assert!(read(&repo, "b.txt").is_none());
    assert_eq!(read(&repo, "c.txt").unwrap(), b"kept\n", "untracked stays");

    undo_hard_reset(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(repo.head(), third);
    assert_eq!(status(&repo), before_status);
    assert_eq!(read(&repo, "a.txt").unwrap(), b"unstaged\r\n");
    assert_eq!(repo.git(&["show", ":a.txt"]), "staged");
    assert_eq!(read(&repo, "b.txt").unwrap(), b"b\n");
}

#[test]
fn hard_reset_snapshot_covers_untracked_files_the_target_contains() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "base");
    repo.git(&["switch", "--quiet", "-c", "other"]);
    let other = repo.commit("x.txt", "tracked there\n", "adds x");
    repo.checkout("main");
    repo.write("x.txt", "my untracked x\n");
    let cli = CliBackend::new(repo.path());
    let snapshot = hard_reset_with_snapshot(&cli, repo.path(), &other).unwrap();
    assert_eq!(read(&repo, "x.txt").unwrap(), b"tracked there\n");
    undo_hard_reset(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(read(&repo, "x.txt").unwrap(), b"my untracked x\n");
    assert_eq!(status(&repo), "?? x.txt");
}

#[test]
fn undo_refuses_when_head_moved() {
    let (repo, base, _) = history();
    let cli = CliBackend::new(repo.path());
    let snapshot = hard_reset_with_snapshot(&cli, repo.path(), &base).unwrap();
    repo.commit("d.txt", "d\n", "later");
    let head = repo.head();
    assert!(undo_hard_reset(&cli, repo.path(), &snapshot).is_err());
    assert_eq!(repo.head(), head);
}

#[cfg(unix)]
#[test]
fn hard_reset_snapshot_keeps_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt as _;
    let (repo, base, _) = history();
    let script = repo.path().join("b.txt");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cli = CliBackend::new(repo.path());
    let snapshot = hard_reset_with_snapshot(&cli, repo.path(), &base).unwrap();
    undo_hard_reset(&cli, repo.path(), &snapshot).unwrap();
    let mode = std::fs::metadata(&script).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0);
}
