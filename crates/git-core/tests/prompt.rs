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
