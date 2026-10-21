// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    ChangeKind, DiffKind, DiffLineKind, DiffRequest, FileChange, FileDiff, GitBackend, GixBackend,
};
use support::TestRepo;

fn backend(repo: &TestRepo) -> GixBackend {
    GixBackend::discover(repo.path()).unwrap()
}

fn request(kind: DiffKind, path: &str) -> DiffRequest {
    DiffRequest {
        kind,
        path: path.into(),
        old_path: None,
    }
}

fn diff(repo: &TestRepo, request: &DiffRequest) -> FileDiff {
    backend(repo).file_diff(request).unwrap()
}

/// Hunks rendered like `git diff` output from the first `@@` on.
fn unified(diff: &FileDiff) -> String {
    let mut out = String::new();
    for hunk in &diff.hunks {
        out.push_str(&hunk.header);
        out.push('\n');
        for line in &hunk.lines {
            out.push(match line.kind {
                DiffLineKind::Context => ' ',
                DiffLineKind::Added => '+',
                DiffLineKind::Removed => '-',
            });
            out.push_str(&line.content);
            out.push('\n');
            if line.missing_newline {
                out.push_str("\\ No newline at end of file\n");
            }
        }
    }
    out
}

/// `git diff` output for the same comparison, from the first `@@` on,
/// without the function context Git appends to hunk headers.
fn git_diff(repo: &TestRepo, args: &[&str]) -> String {
    let mut full = vec![
        "-c",
        "diff.algorithm=histogram",
        "diff",
        "--no-color",
        "--no-ext-diff",
    ];
    full.extend_from_slice(args);
    let output = repo.git(&full);
    let Some(start) = output.find("@@") else {
        return String::new();
    };
    output[start..]
        .lines()
        .map(|line| match line.strip_prefix("@@") {
            Some(rest) => match rest.find("@@") {
                Some(end) => format!("@@{}@@\n", &rest[..end]),
                None => format!("{line}\n"),
            },
            None => format!("{line}\n"),
        })
        .collect()
}

fn numbered(lines: std::ops::Range<u32>) -> String {
    lines.map(|i| format!("line {i}\n")).collect()
}

#[test]
fn commit_file_matches_git() {
    let repo = TestRepo::new();
    repo.commit("file.txt", &numbered(1..30), "Base");
    let changed = numbered(1..30)
        .replace("line 3\n", "three\n")
        .replace("line 20\n", "twenty\nextra\n");
    let id = repo.commit("file.txt", &changed, "Change");

    let result = diff(&repo, &request(DiffKind::Commit(id.clone()), "file.txt"));
    assert_eq!(result.old_path.as_deref(), Some("file.txt"));
    assert_eq!(result.new_path.as_deref(), Some("file.txt"));
    assert!(!result.binary && !result.too_large);
    assert_eq!(result.hunks.len(), 2);
    assert_eq!(
        unified(&result),
        git_diff(&repo, &[&format!("{id}^"), &id, "--", "file.txt"])
    );
}

#[test]
fn root_commit_against_empty_tree() {
    let repo = TestRepo::new();
    let id = repo.commit("a.txt", "hello\n", "Root");
    let result = diff(&repo, &request(DiffKind::Commit(id), "a.txt"));
    assert_eq!(result.old_path, None);
    assert_eq!(unified(&result), "@@ -0,0 +1 @@\n+hello\n");
}

#[test]
fn staged_file_matches_git() {
    let repo = TestRepo::new();
    repo.commit("a.txt", &numbered(1..10), "Base");
    repo.write("a.txt", &numbered(1..10).replace("line 5\n", "staged\n"));
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "worktree only\n");

    let result = diff(&repo, &request(DiffKind::Staged, "a.txt"));
    assert_eq!(
        unified(&result),
        git_diff(&repo, &["--cached", "--", "a.txt"])
    );
    assert!(unified(&result).contains("+staged"));
}

#[test]
fn unstaged_file_matches_git() {
    let repo = TestRepo::new();
    repo.commit("a.txt", &numbered(1..10), "Base");
    repo.write("a.txt", &numbered(1..10).replace("line 2\n", ""));

    let result = diff(&repo, &request(DiffKind::Unstaged, "a.txt"));
    assert_eq!(unified(&result), git_diff(&repo, &["--", "a.txt"]));
}

#[test]
fn untracked_file_against_nothing() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.write("new.txt", "one\ntwo\n");
    let result = diff(&repo, &request(DiffKind::Untracked, "new.txt"));
    assert_eq!(result.old_path, None);
    assert_eq!(result.new_path.as_deref(), Some("new.txt"));
    assert_eq!(unified(&result), "@@ -0,0 +1,2 @@\n+one\n+two\n");
}

#[test]
fn renamed_file_in_commit() {
    let repo = TestRepo::new();
    repo.commit("old.txt", &numbered(1..20), "Base");
    repo.git(&["mv", "old.txt", "new.txt"]);
    repo.write("new.txt", &numbered(1..20).replace("line 10\n", "ten\n"));
    repo.git(&["add", "new.txt"]);
    repo.git(&["commit", "--quiet", "-m", "Rename"]);
    let id = repo.head();

    let changes = backend(&repo).commit_changes(&id).unwrap();
    assert_eq!(
        changes,
        [FileChange {
            path: "new.txt".into(),
            kind: ChangeKind::Renamed,
            old_path: Some("old.txt".into()),
        }]
    );

    let result = diff(
        &repo,
        &DiffRequest {
            kind: DiffKind::Commit(id),
            path: "new.txt".into(),
            old_path: Some("old.txt".into()),
        },
    );
    assert_eq!(result.old_path.as_deref(), Some("old.txt"));
    assert_eq!(result.new_path.as_deref(), Some("new.txt"));
    assert_eq!(result.hunks.len(), 1);
    assert!(unified(&result).contains("-line 10\n+ten\n"));
}

#[test]
fn binary_file() {
    let repo = TestRepo::new();
    repo.write("image.bin", "a\0b");
    repo.git(&["add", "image.bin"]);
    repo.git(&["commit", "--quiet", "-m", "Binary"]);
    let result = diff(&repo, &request(DiffKind::Commit(repo.head()), "image.bin"));
    assert!(result.binary && result.hunks.is_empty());
}

#[test]
fn too_large_file() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.write("big.txt", &"x".repeat(3 * 1024 * 1024));
    let result = diff(&repo, &request(DiffKind::Untracked, "big.txt"));
    assert!(result.too_large && result.hunks.is_empty());
}

#[test]
fn deleted_file() {
    let repo = TestRepo::new();
    repo.commit("gone.txt", "bye\n", "Base");
    repo.git(&["rm", "--quiet", "gone.txt"]);
    repo.git(&["commit", "--quiet", "-m", "Delete"]);
    let id = repo.head();

    let changes = backend(&repo).commit_changes(&id).unwrap();
    assert_eq!(changes[0].kind, ChangeKind::Deleted);
    let result = diff(&repo, &request(DiffKind::Commit(id), "gone.txt"));
    assert_eq!(result.new_path, None);
    assert_eq!(unified(&result), "@@ -1 +0,0 @@\n-bye\n");
}

#[test]
fn empty_file() {
    let repo = TestRepo::new();
    let id = repo.commit("empty.txt", "", "Empty");
    let changes = backend(&repo).commit_changes(&id).unwrap();
    assert_eq!(changes[0].kind, ChangeKind::Added);
    let result = diff(&repo, &request(DiffKind::Commit(id), "empty.txt"));
    assert!(result.hunks.is_empty() && !result.binary);
    assert_eq!(result.new_path.as_deref(), Some("empty.txt"));
}

#[test]
fn file_without_trailing_newline_matches_git() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "one\ntwo", "Base");
    repo.write("a.txt", "one\ntwo\nthree");
    let result = diff(&repo, &request(DiffKind::Unstaged, "a.txt"));
    assert_eq!(unified(&result), git_diff(&repo, &["--", "a.txt"]));
}

#[test]
fn commit_changes_use_first_parent_and_skip_directories() {
    let repo = TestRepo::new();
    repo.commit("base.txt", "base\n", "Base");
    repo.branch("feature");
    repo.checkout("feature");
    repo.commit("dir/feature.txt", "f\n", "Feature");
    repo.checkout("main");
    repo.commit("dir/sub/main.txt", "m\n", "Main");
    let merge = repo.merge("feature", "Merge");

    let changes = backend(&repo).commit_changes(&merge).unwrap();
    let paths: Vec<_> = changes.iter().map(|c| (c.path.as_str(), c.kind)).collect();
    assert_eq!(paths, [("dir/feature.txt", ChangeKind::Added)]);
}

#[test]
fn unknown_commit_is_not_found() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    let result = backend(&repo).commit_changes("0123456789012345678901234567890123456789");
    assert!(matches!(result, Err(git_core::Error::NotFound(_))));
}
