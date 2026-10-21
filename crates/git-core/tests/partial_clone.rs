// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{CliBackend, DiffKind, DiffRequest, Error, GitBackend, GitWriter, GixBackend};
use support::TestRepo;

/// Source repository with history and a blobless clone of it, created with
/// `git clone --filter=blob:none file://…`.
fn blobless_clone() -> (TestRepo, std::path::PathBuf) {
    let source = TestRepo::new();
    source.git(&["config", "uploadpack.allowFilter", "true"]);
    source.commit("a.txt", "one\n", "First");
    source.commit("a.txt", "two\n", "Second");
    let body = "line\n".repeat(20);
    source.commit("old.txt", &body, "Add old");
    source.git(&["mv", "old.txt", "new.txt"]);
    source.write("new.txt", &format!("{body}extra\n"));
    source.git(&["add", "new.txt"]);
    source.git(&["commit", "--quiet", "-m", "Rename with edit"]);

    let clone = source.path().parent().unwrap().join("clone");
    let url = format!("file://{}", source.path().display()).replace('\\', "/");
    source.git(&[
        "clone",
        "--quiet",
        "--filter=blob:none",
        &url,
        clone.to_str().unwrap(),
    ]);
    (source, clone)
}

fn first_commit_diff(clone: &std::path::Path) -> DiffRequest {
    let backend = GixBackend::discover(clone).unwrap();
    let log = backend.log(10).unwrap();
    let first = log.commits.iter().find(|c| c.summary == "First").unwrap();
    DiffRequest {
        kind: DiffKind::Commit(first.id.clone()),
        path: "a.txt".into(),
        old_path: None,
    }
}

#[test]
fn missing_blobs_are_reported_and_fetched_through_git() {
    let (_source, clone) = blobless_clone();
    let backend = GixBackend::discover(&clone).unwrap();
    assert!(backend.is_partial_clone());
    let request = first_commit_diff(&clone);

    let error = backend.file_diff(&request).unwrap_err();
    let Error::MissingObject {
        id,
        partial_clone: true,
    } = &error
    else {
        panic!("unexpected {error:?}");
    };
    assert_eq!(
        error.to_string(),
        "Content not available locally (partial clone)"
    );

    CliBackend::new(&clone).fetch_object(id).unwrap();
    let diff = backend.file_diff(&request).unwrap();
    assert_eq!(diff.hunks.len(), 1);
    assert_eq!(diff.hunks[0].lines[0].content, "one");
}

#[test]
fn commit_file_lists_work_without_the_blobs() {
    let (_source, clone) = blobless_clone();
    let backend = GixBackend::discover(&clone).unwrap();
    let head = backend.head().unwrap().target().unwrap().to_owned();
    let mut paths: Vec<_> = backend
        .commit_changes(&head)
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    paths.sort();
    // The deleted side's blob is not local: no similarity rename detection,
    // the rename shows as deletion plus addition instead of an error.
    assert_eq!(paths, ["new.txt", "old.txt"]);
}

#[test]
fn regular_clones_are_not_partial() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    assert!(!GixBackend::discover(repo.path())
        .unwrap()
        .is_partial_clone());
}
