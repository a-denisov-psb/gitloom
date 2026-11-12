// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{Error, GitBackend, GixBackend, Head, RefKind, Reference};
use support::TestRepo;

fn open(repo: &TestRepo) -> GixBackend {
    GixBackend::discover(repo.path()).unwrap()
}

fn find<'a>(refs: &'a [Reference], name: &str) -> &'a Reference {
    refs.iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("reference {name} missing in {refs:?}"))
}

#[test]
fn linear_history() {
    let repo = TestRepo::new();
    let a = repo.commit("a.txt", "a\n", "First");
    let b = repo.commit("a.txt", "b\n", "Second\n\nBody line one.\nBody line two.");

    let log = open(&repo).log(100).unwrap();
    assert!(!log.truncated);
    let ids: Vec<_> = log.commits.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, [b.as_str(), a.as_str()]);

    let second = &log.commits[0];
    assert_eq!(second.parents, [a]);
    assert_eq!(second.summary, "Second");
    assert_eq!(second.body, "Body line one.\nBody line two.");
    assert_eq!(second.author, "Test Author");
    assert_eq!(second.email, "author@example.com");
    assert_eq!(second.author_time, 1_700_000_060);
    assert_eq!(second.commit_time, 1_700_000_060);
    assert!(log.commits[1].parents.is_empty());
}

#[test]
fn author_and_commit_time_differ() {
    let repo = TestRepo::new();
    repo.git(&[
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "Backdated",
        "--date",
        "1600000000 +0200",
    ]);
    let log = open(&repo).log(10).unwrap();
    assert_eq!(log.commits[0].author_time, 1_600_000_000);
    assert_eq!(log.commits[0].commit_time, 1_700_000_000);
}

#[test]
fn committer_is_only_set_when_it_differs_from_the_author() {
    // The test identities differ: author "Test Author", committer "Test Committer".
    let repo = TestRepo::new();
    repo.commit("a.txt", "1\n", "Applied");
    repo.git(&[
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "Own commit",
        "--author",
        "Test Committer <committer@example.com>",
    ]);
    let log = open(&repo).log(10).unwrap();
    assert_eq!(log.commits[0].committer, "", "same person");
    assert_eq!(log.commits[1].author, "Test Author");
    assert_eq!(log.commits[1].committer, "Test Committer");
}

#[test]
fn log_respects_limit_and_marks_truncation() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "1\n", "One");
    let two = repo.commit("a.txt", "2\n", "Two");
    let three = repo.commit("a.txt", "3\n", "Three");

    let log = open(&repo).log(2).unwrap();
    assert!(log.truncated);
    let ids: Vec<_> = log.commits.iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids, [three, two]);

    assert!(!open(&repo).log(3).unwrap().truncated);
}

#[test]
fn log_includes_all_branches_and_merges() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.branch("feature");
    repo.branch("unmerged");
    repo.checkout("feature");
    let feature = repo.commit("f.txt", "f\n", "Feature");
    repo.checkout("unmerged");
    let unmerged = repo.commit("u.txt", "u\n", "Unmerged");
    repo.checkout("main");
    let main = repo.commit("m.txt", "m\n", "Main");
    let merge = repo.merge("feature", "Merge feature");

    let log = open(&repo).log(100).unwrap();
    assert_eq!(log.commits.len(), 5);
    let merge_commit = log.commits.iter().find(|c| c.id == merge).unwrap();
    assert_eq!(merge_commit.parents, [main, feature]);
    assert!(log.commits.iter().any(|c| c.id == unmerged));
}

#[test]
fn applies_mailmap_to_authors() {
    let repo = TestRepo::new();
    repo.commit(
        ".mailmap",
        "Canonical Name <canonical@example.com> <author@example.com>\n",
        "Add mailmap",
    );
    let log = open(&repo).log(10).unwrap();
    assert_eq!(log.commits[0].author, "Canonical Name");
    assert_eq!(log.commits[0].email, "canonical@example.com");
}

#[test]
fn references_with_head_branches_and_tags() {
    let repo = TestRepo::new();
    let first = repo.commit("a.txt", "a\n", "First");
    repo.tag_annotated("v1.0", "Release 1.0");
    repo.tag("light");
    repo.branch("feature");
    let second = repo.commit("a.txt", "b\n", "Second");

    let backend = open(&repo);
    let refs = backend.references().unwrap();
    assert_eq!(refs.len(), 4, "{refs:?}");

    let main = find(&refs, "main");
    assert_eq!((main.kind, main.is_head), (RefKind::LocalBranch, true));
    assert_eq!(main.target, second);
    let feature = find(&refs, "feature");
    assert_eq!(
        (feature.kind, feature.is_head),
        (RefKind::LocalBranch, false)
    );
    assert_eq!(feature.target, first);

    // Annotated tags are peeled to the commit.
    let annotated = find(&refs, "v1.0");
    assert_eq!(
        (annotated.kind, annotated.target.as_str()),
        (RefKind::Tag, first.as_str())
    );
    assert_eq!(find(&refs, "light").target, first);

    assert_eq!(
        backend.head().unwrap(),
        Head::Branch {
            name: "main".into(),
            target: second
        }
    );
}

#[test]
fn remote_branches_skip_symbolic_head_and_track_upstream() {
    let repo = TestRepo::new();
    let base = repo.commit("a.txt", "a\n", "Base");
    repo.commit("a.txt", "r1\n", "Remote one");
    let remote_two = repo.commit("a.txt", "r2\n", "Remote two");
    repo.git(&["reset", "--quiet", "--hard", &base]);
    repo.commit("a.txt", "l\n", "Local");
    repo.fake_upstream("origin", "main", &remote_two);
    repo.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);

    let refs = open(&repo).references().unwrap();
    assert!(refs.iter().all(|r| r.name != "origin/HEAD"), "{refs:?}");

    let remote = find(&refs, "origin/main");
    assert_eq!(remote.kind, RefKind::RemoteBranch);
    assert_eq!(remote.remote(), Some("origin"));
    assert_eq!(remote.target, remote_two);

    let main = find(&refs, "main");
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));
    assert_eq!(main.ahead_behind, Some((1, 2)));
}

#[test]
fn detached_head() {
    let repo = TestRepo::new();
    let first = repo.commit("a.txt", "a\n", "First");
    repo.commit("a.txt", "b\n", "Second");
    repo.git(&["switch", "--quiet", "--detach", &first]);

    let backend = open(&repo);
    assert_eq!(backend.head().unwrap(), Head::Detached { target: first });
    assert!(backend.references().unwrap().iter().all(|r| !r.is_head));
}

#[test]
fn unborn_head_in_empty_repository() {
    let repo = TestRepo::new();
    let backend = open(&repo);
    assert_eq!(
        backend.head().unwrap(),
        Head::Unborn {
            name: "main".into()
        }
    );
    let log = backend.log(10).unwrap();
    assert!(log.commits.is_empty() && !log.truncated);
    assert!(backend.references().unwrap().is_empty());
    assert!(backend.stashes().unwrap().is_empty());
}

#[test]
fn stashes_newest_first() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "First");
    repo.write("a.txt", "one\n");
    repo.git(&["stash", "push", "--quiet", "-m", "first stash"]);
    repo.write("a.txt", "two\n");
    repo.git(&["stash", "push", "--quiet", "-m", "second stash"]);
    let newest = repo.git(&["rev-parse", "stash@{0}"]);

    let stashes = open(&repo).stashes().unwrap();
    let messages: Vec<_> = stashes.iter().map(|s| s.message.as_str()).collect();
    assert_eq!(messages, ["On main: second stash", "On main: first stash"]);
    assert_eq!((stashes[0].index, stashes[1].index), (0, 1));
    assert_eq!(stashes[0].id, newest);

    // Stashes are no graph tips in M1.
    assert_eq!(open(&repo).log(10).unwrap().commits.len(), 1);
}

#[test]
fn opens_from_subfolder() {
    let repo = TestRepo::new();
    repo.commit("deep/nested/file.txt", "x\n", "Nested");
    let backend = GixBackend::discover(repo.path().join("deep/nested")).unwrap();
    assert_eq!(
        backend.path().canonicalize().unwrap(),
        repo.path().canonicalize().unwrap()
    );
    assert_eq!(backend.name(), "repo");
}

#[test]
fn not_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let result = GixBackend::discover(dir.path());
    assert!(
        matches!(result, Err(Error::NotARepository(_))),
        "{:?}",
        result.err()
    );
}

#[test]
fn missing_path() {
    let dir = tempfile::tempdir().unwrap();
    let result = GixBackend::discover(dir.path().join("missing"));
    assert!(
        matches!(result, Err(Error::NotFound(_))),
        "{:?}",
        result.err()
    );
}

#[test]
fn bare_repository_is_rejected() {
    let repo = TestRepo::new_bare();
    let result = GixBackend::discover(repo.path().join(".git"));
    assert!(
        matches!(result, Err(Error::BareRepository(_))),
        "{:?}",
        result.err()
    );
}
