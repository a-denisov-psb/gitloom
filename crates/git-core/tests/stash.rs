// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{ChangeKind, CliBackend, Error, GitBackend, GitWriter, GixBackend};
use support::TestRepo;

fn isolated() -> TestRepo {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Gitloom Test"]);
    repo.git(&["config", "user.email", "test@example.com"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.commit("a.txt", "a\n", "Base");
    repo
}

fn backend(repo: &TestRepo) -> GixBackend {
    GixBackend::discover(repo.path()).unwrap()
}

#[test]
fn push_includes_untracked_files() {
    let repo = isolated();
    let base = repo.head();
    repo.write("a.txt", "changed\n");
    repo.write("new.txt", "untracked\n");
    CliBackend::new(repo.path()).stash_push(None).unwrap();

    let tree = backend(&repo).working_tree().unwrap();
    assert!(
        tree.is_empty(),
        "{tree:?} / git: {}",
        repo.git(&["status", "--porcelain"])
    );
    let stashes = backend(&repo).stashes().unwrap();
    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].base.as_deref(), Some(base.as_str()));
    assert!(
        stashes[0].message.starts_with("WIP on main"),
        "{}",
        stashes[0].message
    );
    assert!(stashes[0].time > 0);
}

#[test]
fn empty_message_gives_gits_default_message() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "changed\n");
    cli.stash_push(Some("  ")).unwrap();
    let message = backend(&repo).stashes().unwrap()[0].message.clone();
    assert!(message.starts_with("WIP on main: "), "{message}");
    assert_eq!(git_core::stash_label(&message), "WIP on main");
    repo.write("a.txt", "again\n");
    cli.stash_push(Some("try it")).unwrap();
    let message = backend(&repo).stashes().unwrap()[0].message.clone();
    assert_eq!(git_core::stash_label(&message), "try it");
}

#[test]
fn pop_apply_and_drop_by_commit_id() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "first\n");
    cli.stash_push(Some("first")).unwrap();
    repo.write("a.txt", "second\n");
    cli.stash_push(Some("second")).unwrap();
    let stashes = backend(&repo).stashes().unwrap();
    let (newest, older) = (stashes[0].id.clone(), stashes[1].id.clone());

    // The older stash is stash@{1}; it is found by id.
    git_core::apply_stash(&cli, &older, false).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "first\n"
    );
    assert_eq!(backend(&repo).stashes().unwrap().len(), 2);
    repo.git(&["checkout", "--quiet", "--", "a.txt"]);

    cli.stash_drop(&older).unwrap();
    let left: Vec<_> = backend(&repo)
        .stashes()
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(left, std::slice::from_ref(&newest));

    git_core::apply_stash(&cli, &newest, true).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
        "second\n"
    );
    assert!(backend(&repo).stashes().unwrap().is_empty());
}

#[test]
fn pop_with_conflict_keeps_the_stash() {
    let repo = isolated();
    let cli = CliBackend::new(repo.path());
    repo.write("a.txt", "stashed\n");
    cli.stash_push(None).unwrap();
    repo.commit("a.txt", "committed\n", "Conflicting change");
    let id = backend(&repo).stashes().unwrap()[0].id.clone();

    let outcome = git_core::apply_stash(&cli, &id, true).unwrap();
    assert_eq!(outcome, git_core::StashOutcome::Conflicts);
    assert_eq!(backend(&repo).stashes().unwrap().len(), 1);
}

#[test]
fn unknown_stash_is_not_found() {
    let repo = isolated();
    let error = git_core::apply_stash(
        &CliBackend::new(repo.path()),
        "0123456789012345678901234567890123456789",
        true,
    )
    .unwrap_err();
    assert!(matches!(error, Error::NotFound(_)));
}

fn paths(files: &[git_core::FileChange]) -> Vec<(&str, ChangeKind)> {
    files.iter().map(|f| (f.path.as_str(), f.kind)).collect()
}

/// Id of the newest stash entry.
fn top_stash(repo: &TestRepo) -> String {
    repo.git(&["rev-parse", "stash@{0}"])
}

#[test]
fn stash_with_only_untracked_files_lists_them() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.write("new.txt", "new\n");
    repo.write("dir/other.txt", "other\n");
    repo.git(&["stash", "push", "-u", "-m", "untracked only"]);

    let backend = GixBackend::discover(repo.path()).unwrap();
    let id = top_stash(&repo);
    let changes = backend.stash_changes(&id).unwrap();
    assert!(changes.staged.is_empty() && changes.unstaged.is_empty());
    assert_eq!(
        paths(&changes.untracked),
        [
            ("dir/other.txt", ChangeKind::Untracked),
            ("new.txt", ChangeKind::Untracked)
        ]
    );
    assert_eq!(changes.file_count(), 2);

    // The diff of an untracked file is its content against nothing.
    let diff = backend
        .file_diff(&git_core::DiffRequest {
            kind: git_core::DiffKind::Stash {
                id,
                part: git_core::StashPart::Untracked,
            },
            path: "new.txt".into(),
            old_path: None,
        })
        .unwrap();
    assert_eq!(diff.old_path, None);
    assert_eq!(diff.hunks[0].lines[0].content, "new");
}

#[test]
fn stash_with_only_staged_changes() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.write("a.txt", "staged\n");
    repo.git(&["add", "a.txt"]);
    repo.git(&["stash", "push", "-m", "staged only"]);

    let changes = GixBackend::discover(repo.path())
        .unwrap()
        .stash_changes(&top_stash(&repo))
        .unwrap();
    assert_eq!(paths(&changes.staged), [("a.txt", ChangeKind::Modified)]);
    assert!(changes.unstaged.is_empty() && changes.untracked.is_empty());
}

#[test]
fn mixed_stash_is_split_like_the_working_copy() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.commit("b.txt", "b\n", "B");
    // a.txt staged and edited again, b.txt only edited, c.txt untracked.
    repo.write("a.txt", "staged\n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "staged and edited\n");
    repo.write("b.txt", "edited\n");
    repo.write("c.txt", "new\n");
    repo.git(&["stash", "push", "-u", "-m", "mixed"]);

    let backend = GixBackend::discover(repo.path()).unwrap();
    let id = top_stash(&repo);
    let changes = backend.stash_changes(&id).unwrap();
    assert_eq!(paths(&changes.staged), [("a.txt", ChangeKind::Modified)]);
    assert_eq!(
        paths(&changes.unstaged),
        [
            ("a.txt", ChangeKind::Modified),
            ("b.txt", ChangeKind::Modified)
        ]
    );
    assert_eq!(
        paths(&changes.untracked),
        [("c.txt", ChangeKind::Untracked)]
    );
    assert_eq!(changes.file_count(), 3, "a.txt counts once");

    let diff = |part, path: &str| {
        backend
            .file_diff(&git_core::DiffRequest {
                kind: git_core::DiffKind::Stash {
                    id: id.clone(),
                    part,
                },
                path: path.into(),
                old_path: None,
            })
            .unwrap()
    };
    let lines = |d: git_core::FileDiff| -> Vec<String> {
        d.hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .map(|l| l.content.clone())
            .collect()
    };
    assert_eq!(
        lines(diff(git_core::StashPart::Staged, "a.txt")),
        ["a", "staged"]
    );
    assert_eq!(
        lines(diff(git_core::StashPart::Unstaged, "a.txt")),
        ["staged", "staged and edited"]
    );
}
