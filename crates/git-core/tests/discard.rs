// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    discard_all, discard_files, discard_lines, ChangeKind, CliBackend, DiffKind, DiffLineKind,
    DiffRequest, FileChange, GitBackend, GixBackend,
};
use support::TestRepo;

fn change(path: &str, kind: ChangeKind) -> FileChange {
    FileChange {
        path: path.into(),
        kind,
        old_path: None,
    }
}

fn read(repo: &TestRepo, path: &str) -> Vec<u8> {
    std::fs::read(repo.path().join(path)).unwrap()
}

fn blob(repo: &TestRepo, id: &str) -> Vec<u8> {
    repo.git_bytes(&["cat-file", "blob", id])
}

#[test]
fn discard_tracked_file_and_keep_a_snapshot() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "original\n", "Base");
    repo.write("a.txt", "edited\n");
    let snapshot = discard_files(
        &CliBackend::new(repo.path()),
        repo.path(),
        &[change("a.txt", ChangeKind::Modified)],
    )
    .unwrap();
    assert_eq!(read(&repo, "a.txt"), b"original\n");
    let saved = &snapshot.files[0];
    assert_eq!(saved.path, "a.txt");
    assert_eq!(blob(&repo, saved.blob.as_deref().unwrap()), b"edited\n");
}

#[test]
fn discard_untracked_and_binary_files() {
    let repo = TestRepo::new();
    repo.commit("bin.dat", "a\0b", "Base");
    repo.write("bin.dat", "c\0d");
    repo.write("new/untracked.txt", "new\n");
    let snapshot = discard_files(
        &CliBackend::new(repo.path()),
        repo.path(),
        &[
            change("bin.dat", ChangeKind::Modified),
            change("new/untracked.txt", ChangeKind::Untracked),
        ],
    )
    .unwrap();
    assert_eq!(read(&repo, "bin.dat"), b"a\0b");
    assert!(!repo.path().join("new/untracked.txt").exists());
    assert_eq!(
        blob(&repo, snapshot.files[0].blob.as_deref().unwrap()),
        b"c\0d"
    );
    assert_eq!(
        blob(&repo, snapshot.files[1].blob.as_deref().unwrap()),
        b"new\n"
    );
}

#[test]
fn deleted_file_comes_back_and_snapshot_has_no_blob() {
    let repo = TestRepo::new();
    repo.commit("gone.txt", "here\n", "Base");
    std::fs::remove_file(repo.path().join("gone.txt")).unwrap();
    let snapshot = discard_files(
        &CliBackend::new(repo.path()),
        repo.path(),
        &[change("gone.txt", ChangeKind::Deleted)],
    )
    .unwrap();
    assert_eq!(read(&repo, "gone.txt"), b"here\n");
    assert_eq!(snapshot.files[0].blob, None);
}

#[test]
fn staged_changes_survive_discard_all() {
    let repo = TestRepo::new();
    repo.write("staged.txt", "s\n");
    repo.git(&["add", "staged.txt"]);
    repo.commit("a.txt", "a\n", "Base");
    repo.write("staged.txt", "staged change\n");
    repo.git(&["add", "staged.txt"]);
    repo.write("a.txt", "unstaged change\n");
    repo.write("junk.txt", "junk\n");
    let backend = GixBackend::discover(repo.path()).unwrap();
    let unstaged = backend.working_tree().unwrap().unstaged;
    discard_all(&CliBackend::new(repo.path()), repo.path(), &unstaged).unwrap();
    let tree = backend.working_tree().unwrap();
    assert!(tree.unstaged.is_empty(), "{tree:?}");
    assert_eq!(tree.staged.len(), 1);
    assert_eq!(read(&repo, "staged.txt"), b"staged change\n");
}

#[cfg(unix)]
#[test]
fn snapshot_records_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt as _;
    let repo = TestRepo::new();
    repo.commit("run.sh", "echo 1\n", "Base");
    let path = repo.path().join("run.sh");
    std::fs::write(&path, "echo 2\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let snapshot = discard_files(
        &CliBackend::new(repo.path()),
        repo.path(),
        &[change("run.sh", ChangeKind::Modified)],
    )
    .unwrap();
    assert!(snapshot.files[0].executable);
}

/// Discards the changed lines for which `pick(line content)` is true.
fn discard_matching(repo: &TestRepo, path: &str, pick: impl Fn(&str) -> bool) {
    let backend = GixBackend::discover(repo.path()).unwrap();
    let diff = backend
        .file_diff(&DiffRequest {
            kind: DiffKind::Unstaged,
            path: path.into(),
            old_path: None,
        })
        .unwrap();
    let content = backend.worktree_content(path).unwrap().unwrap();
    let selected = |h: usize, l: usize| {
        let line = &diff.hunks[h].lines[l];
        line.kind != DiffLineKind::Context && pick(&line.content)
    };
    discard_lines(
        &CliBackend::new(repo.path()),
        repo.path(),
        &diff,
        &content,
        selected,
    )
    .unwrap();
}

#[test]
fn discard_selected_lines() {
    let repo = TestRepo::new();
    repo.commit("f.txt", "one\ntwo\nthree\n", "Base");
    repo.write("f.txt", "one\nTWO\nthree\nfour\n");
    discard_matching(&repo, "f.txt", |line| line == "four");
    assert_eq!(read(&repo, "f.txt"), b"one\nTWO\nthree\n");
    discard_matching(&repo, "f.txt", |_| true);
    assert_eq!(read(&repo, "f.txt"), b"one\ntwo\nthree\n");
}

#[test]
fn discarding_lines_keeps_crlf_in_the_working_tree() {
    let repo = TestRepo::new();
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.commit("w.txt", "a\r\nb\r\nc\r\n", "Base");
    repo.write("w.txt", "a\r\nB\r\nc\r\nd\r\n");
    discard_matching(&repo, "w.txt", |line| line == "d");
    assert_eq!(read(&repo, "w.txt"), b"a\r\nB\r\nc\r\n");
}
