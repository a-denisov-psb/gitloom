// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{GitBackend, GixBackend};
use support::TestRepo;

/// Config written by `git` after the backend was opened (new remotes,
/// upstreams set by `push -u`) must show up without reopening the tab.
#[test]
fn config_changes_are_seen_by_an_open_backend() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert!(backend.remotes().unwrap().is_empty());

    repo.fake_upstream("origin", "main", "HEAD");
    assert_eq!(backend.remotes().unwrap(), ["origin"]);
    let main = backend
        .references()
        .unwrap()
        .into_iter()
        .find(|r| r.name == "main")
        .unwrap();
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));
}
