//! A remote defined in the global config (M6-18). Its own test binary:
//! `GIT_CONFIG_GLOBAL` is set for the whole process.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{missing_remote, CliBackend, ConfigScope, GitBackend, GitWriter, GixBackend};
use support::TestRepo;

#[test]
fn a_remote_in_the_global_config_is_marked_and_followed() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["remote", "add", "origin", "https://example.com/o.git"]);
    let global = repo.path().join("..").join("global.gitconfig");
    let section = "[remote \"glob\"]\n\turl = https://example.com/g.git\n\
                   \tfetch = +refs/heads/*:refs/remotes/glob/*\n";
    std::fs::write(&global, section).unwrap();
    // Read by gix and by the `git` the CLI backend runs.
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);

    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(backend.remotes().unwrap(), ["glob", "origin"]);
    assert_eq!(
        backend.external_remotes().unwrap(),
        [("glob".to_owned(), ConfigScope::Global)]
    );
    assert_eq!(
        backend.remote_url("glob").unwrap().as_deref(),
        Some("https://example.com/g.git")
    );

    // Git cannot remove it from the repository: "No such remote".
    let error = CliBackend::new(repo.path())
        .remove_remote("glob")
        .unwrap_err()
        .to_string();
    assert_eq!(missing_remote(&error).as_deref(), Some("glob"), "{error}");

    // A local section makes it a repository remote.
    repo.git(&["config", "remote.glob.pushurl", "https://example.com/p.git"]);
    assert!(backend.external_remotes().unwrap().is_empty());
    repo.git(&["config", "--unset", "remote.glob.pushurl"]);
    assert_eq!(
        backend.external_remotes().unwrap(),
        [("glob".to_owned(), ConfigScope::Global)]
    );

    // Removed from the global config: the open backend follows.
    std::fs::write(&global, "").unwrap();
    assert_eq!(backend.remotes().unwrap(), ["origin"]);
    assert!(backend.external_remotes().unwrap().is_empty());
}
