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

/// Status, staged diff and unstaged diff: the full working copy state.
fn state(repo: &TestRepo) -> (String, String, String) {
    (
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        repo.git(&["diff", "--cached"]),
        repo.git(&["diff"]),
    )
}

#[test]
fn discard_everything_including_staged_and_undo_it() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "base\n", "Base");
    repo.commit("b.txt", "bee\n", "B");
    repo.commit("e.txt", "eee\n", "E");
    // Staged and modified again, a staged new file, a staged rename,
    // an untracked file and an unstaged change.
    repo.write("a.txt", "staged\n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "staged and edited\n");
    repo.write("c.txt", "new\n");
    repo.git(&["add", "c.txt"]);
    repo.git(&["mv", "b.txt", "d.txt"]);
    repo.write("u.txt", "untracked\n");
    repo.write("e.txt", "edited\n");
    let before = state(&repo);

    let cli = CliBackend::new(repo.path());
    let tree = GixBackend::discover(repo.path())
        .unwrap()
        .working_tree()
        .unwrap();
    let snapshot = git_core::discard_everything(&cli, repo.path(), &tree).unwrap();
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
    assert_eq!(read(&repo, "a.txt"), b"base\n");
    assert_eq!(read(&repo, "b.txt"), b"bee\n");
    assert!(!repo.path().join("c.txt").exists());
    assert!(!repo.path().join("d.txt").exists());
    assert!(!repo.path().join("u.txt").exists());

    // Undo brings back index and working tree; redo discards again.
    let redo = git_core::restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(state(&repo), before);
    git_core::restore_snapshot(&cli, repo.path(), &redo).unwrap();
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
}

#[test]
fn discard_everything_before_the_first_commit() {
    let repo = TestRepo::new();
    repo.write("a.txt", "new\n");
    repo.git(&["add", "a.txt"]);
    repo.write("u.txt", "untracked\n");
    let before = state_unborn(&repo);

    let cli = CliBackend::new(repo.path());
    let tree = GixBackend::discover(repo.path())
        .unwrap()
        .working_tree()
        .unwrap();
    let snapshot = git_core::discard_everything(&cli, repo.path(), &tree).unwrap();
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
    git_core::restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(state_unborn(&repo), before);
}

/// `git diff --cached` needs HEAD; compare status and index content instead.
fn state_unborn(repo: &TestRepo) -> (String, String) {
    (
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        repo.git(&["ls-files", "--stage"]),
    )
}

#[test]
fn discard_one_staged_file_and_undo_it() {
    // "Discard changes" of a staged file (M13-07): the app passes the file
    // and its unstaged entry; other files keep their changes.
    let repo = TestRepo::new();
    repo.commit("a.txt", "base\n", "Base");
    repo.commit("e.txt", "eee\n", "E");
    repo.write("a.txt", "staged\n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "staged and edited\n");
    repo.write("c.txt", "new\n");
    repo.git(&["add", "c.txt"]);
    repo.write("e.txt", "edited\n");
    let before = state(&repo);
    let cli = CliBackend::new(repo.path());
    let full = GixBackend::discover(repo.path())
        .unwrap()
        .working_tree()
        .unwrap();
    let only = |path: &str| git_core::WorkingTree {
        staged: full
            .staged
            .iter()
            .filter(|f| f.path == path)
            .cloned()
            .collect(),
        unstaged: full
            .unstaged
            .iter()
            .filter(|f| f.path == path)
            .cloned()
            .collect(),
    };

    let snapshot = git_core::discard_everything(&cli, repo.path(), &only("a.txt")).unwrap();
    assert_eq!(read(&repo, "a.txt"), b"base\n");
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        "A  c.txt\n M e.txt"
    );
    let new_file = git_core::discard_everything(&cli, repo.path(), &only("c.txt")).unwrap();
    assert!(!repo.path().join("c.txt").exists());
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=all"]),
        "M e.txt"
    );

    // Undo in reverse order restores index and working tree.
    git_core::restore_snapshot(&cli, repo.path(), &new_file).unwrap();
    git_core::restore_snapshot(&cli, repo.path(), &snapshot).unwrap();
    assert_eq!(state(&repo), before);
}
