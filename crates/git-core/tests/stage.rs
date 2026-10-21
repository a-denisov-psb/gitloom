// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{ChangeKind, CliBackend, GitBackend, GitWriter, GixBackend, WorkingTree};
use support::TestRepo;

fn status(repo: &TestRepo) -> WorkingTree {
    GixBackend::discover(repo.path())
        .unwrap()
        .working_tree()
        .unwrap()
}

fn writer(repo: &TestRepo) -> CliBackend {
    CliBackend::new(repo.path())
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|p| p.to_string()).collect()
}

fn kinds(changes: &[git_core::FileChange]) -> Vec<(&str, ChangeKind)> {
    changes.iter().map(|c| (c.path.as_str(), c.kind)).collect()
}

fn base() -> TestRepo {
    let repo = TestRepo::new();
    repo.write("keep.txt", "keep\n");
    repo.git(&["add", "keep.txt"]);
    repo.commit("a.txt", "a\n", "Base");
    repo
}

#[test]
fn stage_and_unstage_modified_file() {
    let repo = base();
    repo.write("a.txt", "changed\n");
    writer(&repo).stage(&paths(&["a.txt"])).unwrap();
    assert_eq!(
        kinds(&status(&repo).staged),
        [("a.txt", ChangeKind::Modified)]
    );
    writer(&repo).unstage(&paths(&["a.txt"])).unwrap();
    let tree = status(&repo);
    assert!(tree.staged.is_empty());
    assert_eq!(kinds(&tree.unstaged), [("a.txt", ChangeKind::Modified)]);
}

#[test]
fn stage_untracked_and_deleted_files() {
    let repo = base();
    repo.write("new.txt", "new\n");
    std::fs::remove_file(repo.path().join("keep.txt")).unwrap();
    writer(&repo)
        .stage(&paths(&["new.txt", "keep.txt"]))
        .unwrap();
    assert_eq!(
        kinds(&status(&repo).staged),
        [
            ("keep.txt", ChangeKind::Deleted),
            ("new.txt", ChangeKind::Added)
        ]
    );
}

#[test]
fn unstage_rename_restores_both_paths() {
    let repo = base();
    repo.git(&["mv", "a.txt", "b.txt"]);
    writer(&repo).unstage(&paths(&["b.txt", "a.txt"])).unwrap();
    let tree = status(&repo);
    assert!(tree.staged.is_empty(), "{tree:?}");
    assert_eq!(
        kinds(&tree.unstaged),
        [
            ("a.txt", ChangeKind::Deleted),
            ("b.txt", ChangeKind::Untracked)
        ]
    );
}

#[test]
fn staging_a_conflict_marks_it_resolved() {
    let repo = base();
    repo.branch("other");
    repo.commit("a.txt", "main\n", "Main");
    repo.checkout("other");
    repo.commit("a.txt", "other\n", "Other");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "--quiet", "other"]));
    assert_eq!(
        kinds(&status(&repo).unstaged),
        [("a.txt", ChangeKind::Conflicted)]
    );

    repo.write("a.txt", "resolved\n");
    writer(&repo).stage(&paths(&["a.txt"])).unwrap();
    let tree = status(&repo);
    assert_eq!(kinds(&tree.staged), [("a.txt", ChangeKind::Modified)]);
    assert!(tree.unstaged.is_empty());
}

#[test]
fn repository_without_commits() {
    let repo = TestRepo::new();
    repo.write("first.txt", "1\n");
    repo.write("second.txt", "2\n");
    let cli = writer(&repo);
    assert!(!cli.has_head().unwrap());
    cli.stage(&paths(&["first.txt", "second.txt"])).unwrap();
    assert_eq!(status(&repo).staged.len(), 2);
    cli.unstage(&paths(&["first.txt"])).unwrap();
    assert_eq!(
        kinds(&status(&repo).staged),
        [("second.txt", ChangeKind::Added)]
    );
    cli.unstage_all().unwrap();
    let tree = status(&repo);
    assert!(tree.staged.is_empty());
    assert_eq!(tree.unstaged.len(), 2);
}

#[test]
fn stage_all_and_unstage_all() {
    let repo = base();
    repo.write("a.txt", "changed\n");
    repo.write("new.txt", "new\n");
    let cli = writer(&repo);
    cli.stage_all().unwrap();
    assert_eq!(status(&repo).staged.len(), 2);
    cli.unstage_all().unwrap();
    let tree = status(&repo);
    assert!(tree.staged.is_empty());
    assert_eq!(tree.unstaged.len(), 2);
}

#[test]
fn file_names_are_not_glob_patterns() {
    let repo = base();
    repo.write("star*.txt", "glob\n");
    repo.write("starX.txt", "other\n");
    writer(&repo).stage(&paths(&["star*.txt"])).unwrap();
    assert_eq!(
        kinds(&status(&repo).staged),
        [("star*.txt", ChangeKind::Added)]
    );
}

#[test]
fn submodule_is_staged_as_one_entry() {
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
    repo.git_in(
        &repo.path().join("sub"),
        &["commit", "--allow-empty", "--quiet", "-m", "Move"],
    );
    assert_eq!(
        kinds(&status(&repo).unstaged),
        [("sub", ChangeKind::Modified)]
    );
    writer(&repo).stage(&paths(&["sub"])).unwrap();
    assert_eq!(
        kinds(&status(&repo).staged),
        [("sub", ChangeKind::Modified)]
    );
}

#[test]
fn many_paths_go_through_stdin() {
    let repo = base();
    let names: Vec<String> = (0..3000).map(|i| format!("dir/file-{i:04}.txt")).collect();
    for name in &names {
        repo.write(name, "x\n");
    }
    writer(&repo).stage(&names).unwrap();
    assert_eq!(status(&repo).staged.len(), 3000);
}
