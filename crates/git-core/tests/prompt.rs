//! Data of the commit message prompt (M10-07).

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::prompt::{build, gather, Omitted, DEFAULT_INSTRUCTION};
use git_core::CliBackend;
use support::TestRepo;

#[test]
fn staged_files_counts_diffs_and_omissions() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "one\ntwo\n", "First commit");
    repo.commit("b.txt", "b\n", "Second commit");
    repo.write("a.txt", "one\nTWO\nthree\n");
    std::fs::write(repo.path().join("bin.dat"), [0u8, 1, 2, 0, 255]).unwrap();
    repo.write("big.txt", &"line\n".repeat(60_000));
    repo.write("unstaged.txt", "not staged\n");
    repo.git(&["add", "a.txt", "bin.dat", "big.txt"]);
    repo.git(&["rm", "--quiet", "b.txt"]);

    let input = gather(&CliBackend::new(repo.path())).unwrap();
    assert_eq!(input.branch, "main");
    assert_eq!(input.recent, ["Second commit", "First commit"]);
    let files: Vec<(&str, &str)> = input
        .files
        .iter()
        .map(|f| (f.status.as_str(), f.path.as_str()))
        .collect();
    assert_eq!(
        files,
        [
            ("M", "a.txt"),
            ("D", "b.txt"),
            ("A", "big.txt"),
            ("A", "bin.dat")
        ]
    );
    let a = &input.files[0];
    assert_eq!(a.counts, Some((2, 1)));
    assert!(a.diff.as_ref().unwrap().contains("+three"));
    assert_eq!(input.files[2].diff, Err(Omitted::TooLarge));
    assert_eq!(input.files[3].diff, Err(Omitted::Binary));
    assert_eq!(input.files[3].counts, None);

    let prompt = build(DEFAULT_INSTRUCTION, None, &input);
    assert!(prompt.text.contains("M a.txt (+2 -1)"));
    assert!(!prompt.text.contains("unstaged.txt"));
}

#[test]
fn unborn_branch_has_no_recent_commits() {
    let repo = TestRepo::new();
    repo.write("a.txt", "a\n");
    repo.git(&["add", "a.txt"]);
    let input = gather(&CliBackend::new(repo.path())).unwrap();
    assert!(input.recent.is_empty());
    assert_eq!(input.files.len(), 1);
}

/// M15-06: a commit's prompt carries its own diff against the first parent
/// (root commits against the empty tree), not the staged changes.
#[test]
fn commit_source_uses_the_commits_diff() {
    use git_core::prompt::{gather_from, PromptSource};
    let repo = TestRepo::new();
    let root = repo.commit("a.txt", "root line\n", "Root commit");
    repo.branch("topic");
    let second = repo.commit("a.txt", "root line\nsecond line\n", "Second commit");
    repo.checkout("topic");
    repo.commit("t.txt", "topic file\n", "Topic commit");
    repo.checkout("main");
    let merge = repo.merge("topic", "Merge topic");
    repo.write("a.txt", "STAGED MARKER\n");
    repo.git(&["add", "a.txt"]);
    let cli = CliBackend::new(repo.path());

    let input = gather_from(&cli, &PromptSource::Commit(second.clone())).unwrap();
    assert_eq!(input.recent, ["Root commit"]);
    let files: Vec<_> = input
        .files
        .iter()
        .map(|f| (f.status.as_str(), f.path.as_str(), f.counts))
        .collect();
    assert_eq!(files, [("M", "a.txt", Some((1, 0)))]);
    let prompt = build(DEFAULT_INSTRUCTION, None, &input);
    assert!(prompt.text.contains("+second line"), "{}", prompt.text);
    assert!(!prompt.text.contains("STAGED MARKER"));
    assert!(prompt
        .text
        .contains(&format!("Diff of commit {}:", &second[..7])));
    assert!(!prompt.text.contains("Staged diff:"));

    // Root commit: against the empty tree, no commits before it.
    let input = gather_from(&cli, &PromptSource::Commit(root)).unwrap();
    assert!(input.recent.is_empty());
    let prompt = build(DEFAULT_INSTRUCTION, None, &input);
    assert!(prompt.text.contains("A a.txt (+1 -0)"), "{}", prompt.text);
    assert!(prompt.text.contains("+root line"));

    // Merge commit: against its first parent, so the topic's file.
    let input = gather_from(&cli, &PromptSource::Commit(merge)).unwrap();
    assert_eq!(input.recent, ["Second commit", "Root commit"]);
    let prompt = build(DEFAULT_INSTRUCTION, None, &input);
    assert!(prompt.text.contains("A t.txt (+1 -0)"), "{}", prompt.text);
    assert!(prompt.text.contains("+topic file"));
    assert!(!prompt.text.contains("a.txt"));

    // The staged source is unchanged.
    let staged = build(DEFAULT_INSTRUCTION, None, &gather(&cli).unwrap());
    assert!(staged.text.contains("STAGED MARKER"));
    assert!(staged.text.contains("Staged diff:"));
}
