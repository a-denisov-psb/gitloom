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

/// `git remote add/rename/remove` in a terminal while the backend is open.
#[test]
fn remote_changes_outside_are_seen() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    let backend = GixBackend::discover(repo.path()).unwrap();
    repo.git(&["remote", "add", "github-fc", "https://example.com/a.git"]);
    assert_eq!(backend.remotes().unwrap(), ["github-fc"]);
    repo.git(&["remote", "rename", "github-fc", "origin"]);
    assert_eq!(backend.remotes().unwrap(), ["origin"]);
    repo.git(&["remote", "set-url", "origin", "https://example.com/b.git"]);
    assert_eq!(
        backend.remote_url("origin").unwrap().as_deref(),
        Some("https://example.com/b.git")
    );
    repo.git(&["remote", "remove", "origin"]);
    assert!(backend.remotes().unwrap().is_empty());
}

/// Several config writes within one timestamp tick (Windows updates file
/// times in steps of about 16 ms; `git remote rename` writes the config
/// several times): a change that keeps the modification time must still be
/// seen. Also covers a rewrite via a temporary file and rename, as
/// `git filter-repo` and editors do.
#[test]
fn a_config_rewrite_with_the_same_modification_time_is_seen() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["remote", "add", "github-fc", "https://example.com/a.git"]);
    let config = repo.path().join(".git").join("config");
    let mtime = std::fs::metadata(&config).unwrap().modified().unwrap();
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(backend.remotes().unwrap(), ["github-fc"]);

    // Rewrite without the remote, renamed into place with the old time.
    let text = std::fs::read_to_string(&config).unwrap();
    let start = text.find("[remote \"github-fc\"]").unwrap();
    let rewritten = text[..start].to_owned();
    let temp = repo.path().join(".git").join("config.tmp");
    std::fs::write(&temp, &rewritten).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&temp)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    std::fs::rename(&temp, &config).unwrap();
    assert_eq!(
        std::fs::metadata(&config).unwrap().modified().unwrap(),
        mtime
    );
    assert!(
        backend.remotes().unwrap().is_empty(),
        "stale remote after a rewrite with the same modification time"
    );
}
