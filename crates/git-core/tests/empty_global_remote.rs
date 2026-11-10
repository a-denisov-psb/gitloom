//! An empty `[remote "…"]` section in the global config is not a remote
//! (M6-20). Its own test binary: `GIT_CONFIG_GLOBAL` is set for the whole
//! process.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{GitBackend, GixBackend};
use support::TestRepo;

#[test]
fn an_empty_global_section_is_not_a_remote() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["remote", "add", "origin", "https://example.com/o.git"]);
    let global = repo.path().join("..").join("global.gitconfig");
    std::fs::write(&global, "[remote \"github-fc\"]\n").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);

    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(backend.remotes().unwrap(), ["origin"]);
    assert!(backend.external_remotes().unwrap().is_empty());

    // With a key it is a remote from the global config.
    std::fs::write(
        &global,
        "[remote \"github-fc\"]\n\turl = https://example.com/g.git\n",
    )
    .unwrap();
    assert_eq!(backend.remotes().unwrap(), ["github-fc", "origin"]);
    assert_eq!(backend.external_remotes().unwrap().len(), 1);
}
