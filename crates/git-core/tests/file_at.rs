// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{commit_id, file_at_commit, CliBackend, CommitFile};
use support::TestRepo;

fn content(repo: &TestRepo, rev: &str, path: &str) -> CommitFile {
    file_at_commit(&CliBackend::new(repo.path()), rev, path).unwrap()
}

#[test]
fn file_of_a_branch_that_is_not_checked_out() {
    // Finding #53: README.md exists only on `feature-a`, `feature-b` is
    // checked out.
    let repo = TestRepo::new();
    repo.commit("a.txt", "base\n", "base");
    repo.branch("feature-a");
    repo.branch("feature-b");
    repo.checkout("feature-a");
    let added = repo.commit("README.md", "# Hello\n", "add readme");
    repo.checkout("feature-b");
    repo.commit("b.txt", "b\n", "other");
    assert!(!repo.path().join("README.md").exists());
    assert_eq!(
        content(&repo, &added, "README.md"),
        CommitFile::Content(b"# Hello\n".to_vec())
    );
}

#[test]
fn content_differs_from_the_working_tree() {
    let repo = TestRepo::new();
    let old = repo.commit("a.txt", "old\n", "one");
    repo.commit("a.txt", "new\n", "two");
    repo.write("a.txt", "modified\n");
    assert_eq!(
        content(&repo, &old, "a.txt"),
        CommitFile::Content(b"old\n".to_vec())
    );
}

#[test]
fn line_endings_follow_autocrlf() {
    let repo = TestRepo::new();
    let id = repo.commit("a.txt", "one\ntwo\n", "lf");
    repo.git(&["config", "core.autocrlf", "true"]);
    assert_eq!(
        content(&repo, &id, "a.txt"),
        CommitFile::Content(b"one\r\ntwo\r\n".to_vec())
    );
}

#[test]
fn binary_file_is_byte_identical() {
    let repo = TestRepo::new();
    let bytes: Vec<u8> = (0..=255u8).chain([0, 0, 13, 10, 255]).collect();
    std::fs::write(repo.path().join("image.bin"), &bytes).unwrap();
    repo.git(&["add", "image.bin"]);
    repo.git(&["commit", "-q", "-m", "binary"]);
    repo.git(&["config", "core.autocrlf", "true"]);
    assert_eq!(
        content(&repo, "HEAD", "image.bin"),
        CommitFile::Content(bytes)
    );
}

#[test]
fn large_file_has_no_size_limit() {
    let repo = TestRepo::new();
    let big = "0123456789abcdef\n".repeat(200_000);
    assert!(big.len() > 3 * 1024 * 1024);
    let id = repo.commit("big.txt", &big, "big");
    assert_eq!(
        content(&repo, &id, "big.txt"),
        CommitFile::Content(big.into_bytes())
    );
}

#[test]
fn missing_path_folder_and_nested_file() {
    let repo = TestRepo::new();
    let id = repo.commit("src/deep/a.rs", "fn a() {}\n", "nested");
    assert_eq!(content(&repo, &id, "nope.txt"), CommitFile::Missing);
    assert_eq!(content(&repo, &id, "src/deep/b.rs"), CommitFile::Missing);
    assert_eq!(content(&repo, &id, "src"), CommitFile::Folder);
    assert_eq!(
        content(&repo, &id, "src/deep/a.rs"),
        CommitFile::Content(b"fn a() {}\n".to_vec())
    );
    // A pathspec pattern matches only the literal name.
    assert_eq!(content(&repo, &id, "src/deep/*.rs"), CommitFile::Missing);
}

#[test]
fn deleted_file_is_read_from_the_first_parent() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "before\n", "add");
    repo.git(&["rm", "-q", "a.txt"]);
    repo.git(&["commit", "-q", "-m", "delete"]);
    let deleted = repo.head();
    assert_eq!(content(&repo, &deleted, "a.txt"), CommitFile::Missing);
    let parent = format!("{deleted}^1");
    assert_eq!(
        content(&repo, &parent, "a.txt"),
        CommitFile::Content(b"before\n".to_vec())
    );
    let cli = CliBackend::new(repo.path());
    assert_eq!(
        commit_id(&cli, &parent).unwrap(),
        Some(repo.parents(&deleted)[0].clone())
    );
}

#[test]
fn root_commit_has_no_parent() {
    let repo = TestRepo::new();
    let root = repo.commit("a.txt", "a\n", "root");
    let cli = CliBackend::new(repo.path());
    assert_eq!(commit_id(&cli, &format!("{root}^1")).unwrap(), None);
    assert_eq!(commit_id(&cli, "HEAD").unwrap(), Some(root));
}

#[test]
fn submodule_entry_has_no_content() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "base");
    let head = repo.head();
    // A gitlink without a checked-out submodule is enough for the tree entry.
    repo.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{head},lib"),
    ]);
    repo.git(&["commit", "-q", "-m", "submodule"]);
    assert_eq!(content(&repo, "HEAD", "lib"), CommitFile::Submodule);
}
