// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{ChangeKind, FileChange, GitBackend, GixBackend, WorkingTree};
use support::TestRepo;

fn status(repo: &TestRepo) -> WorkingTree {
    GixBackend::discover(repo.path())
        .unwrap()
        .working_tree()
        .unwrap()
}

fn change(path: &str, kind: ChangeKind) -> FileChange {
    FileChange {
        path: path.into(),
        kind,
        old_path: None,
    }
}

/// Repository with a committed `tracked.txt` and `other.txt`.
fn base() -> TestRepo {
    let repo = TestRepo::new();
    repo.write("other.txt", "other\n");
    repo.git(&["add", "other.txt"]);
    repo.commit("tracked.txt", "one\ntwo\nthree\n", "Base");
    repo
}

#[test]
fn clean_worktree() {
    let tree = status(&base());
    assert!(tree.is_empty(), "{tree:?}");
}

#[test]
fn modified_file_is_unstaged() {
    let repo = base();
    repo.write("tracked.txt", "changed\n");
    let tree = status(&repo);
    assert_eq!(tree.unstaged, [change("tracked.txt", ChangeKind::Modified)]);
    assert!(tree.staged.is_empty());
}

#[test]
fn added_file_is_staged() {
    let repo = base();
    repo.write("new.txt", "new\n");
    repo.git(&["add", "new.txt"]);
    let tree = status(&repo);
    assert_eq!(tree.staged, [change("new.txt", ChangeKind::Added)]);
    assert!(tree.unstaged.is_empty());
}

#[test]
fn deleted_file_unstaged_and_staged() {
    let repo = base();
    std::fs::remove_file(repo.path().join("tracked.txt")).unwrap();
    repo.git(&["rm", "--quiet", "other.txt"]);
    let tree = status(&repo);
    assert_eq!(tree.unstaged, [change("tracked.txt", ChangeKind::Deleted)]);
    assert_eq!(tree.staged, [change("other.txt", ChangeKind::Deleted)]);
}

#[test]
fn renamed_file_is_staged_rename() {
    let repo = base();
    repo.git(&["mv", "tracked.txt", "moved.txt"]);
    let tree = status(&repo);
    assert_eq!(
        tree.staged,
        [FileChange {
            path: "moved.txt".into(),
            kind: ChangeKind::Renamed,
            old_path: Some("tracked.txt".into()),
        }]
    );
    assert!(tree.unstaged.is_empty(), "{tree:?}");
}

#[test]
fn untracked_files_are_listed_individually() {
    let repo = base();
    repo.write("dir/a.txt", "a\n");
    repo.write("dir/b.txt", "b\n");
    let tree = status(&repo);
    assert_eq!(
        tree.unstaged,
        [
            change("dir/a.txt", ChangeKind::Untracked),
            change("dir/b.txt", ChangeKind::Untracked),
        ]
    );
}

#[test]
fn ignored_files_are_not_listed() {
    let repo = TestRepo::new();
    repo.commit(".gitignore", "*.log\nbuild/\n", "Ignore");
    repo.write("debug.log", "x\n");
    repo.write("build/out.bin", "x\n");
    assert!(status(&repo).is_empty());
}

#[test]
fn staged_and_modified_again_appears_in_both_lists() {
    let repo = base();
    repo.write("tracked.txt", "staged\n");
    repo.git(&["add", "tracked.txt"]);
    repo.write("tracked.txt", "modified again\n");
    let tree = status(&repo);
    assert_eq!(tree.staged, [change("tracked.txt", ChangeKind::Modified)]);
    assert_eq!(tree.unstaged, [change("tracked.txt", ChangeKind::Modified)]);
}

#[test]
fn conflict_after_failed_merge() {
    let repo = base();
    repo.branch("other");
    repo.commit("tracked.txt", "main side\n", "Main change");
    repo.checkout("other");
    repo.commit("tracked.txt", "other side\n", "Other change");
    repo.checkout("main");
    assert!(
        !repo.try_git(&["merge", "--quiet", "other"]),
        "merge should conflict"
    );

    let tree = status(&repo);
    assert_eq!(
        tree.unstaged,
        [change("tracked.txt", ChangeKind::Conflicted)]
    );
    assert!(tree.staged.is_empty(), "{tree:?}");
}

#[test]
fn submodule_is_a_single_entry() {
    let sub = TestRepo::new();
    sub.commit("inner.txt", "inner\n", "Inner");

    let repo = base();
    repo.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        sub.path().to_str().unwrap(),
        "sub",
    ]);
    repo.git(&["commit", "--quiet", "-m", "Add submodule"]);
    std::fs::write(repo.path().join("sub/inner.txt"), "changed\n").unwrap();

    let tree = status(&repo);
    assert_eq!(tree.unstaged, [change("sub", ChangeKind::Modified)]);
    assert!(tree.staged.is_empty());
}
