//! Empty `[remote "…"]` sections are not remotes (M6-20): Git ignores a
//! section without keys (`git remote` does not list it), so Gitloom does
//! too, and Gitloom's own remote operations never leave one behind.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{CliBackend, GitBackend, GitWriter, GixBackend};
use support::TestRepo;

/// Names of the `[remote "…"]` sections of `.git/config` that have no key.
fn empty_remote_sections(repo: &TestRepo) -> Vec<String> {
    let text = std::fs::read_to_string(repo.path().join(".git").join("config")).unwrap();
    let mut empty = Vec::new();
    let mut current: Option<(String, bool)> = None;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            if let Some((name, false)) = current.take() {
                empty.push(name);
            }
            current = line
                .strip_prefix("[remote \"")
                .and_then(|rest| rest.strip_suffix("\"]"))
                .map(|name| (name.to_owned(), false));
        } else if let Some((_, keys)) = current.as_mut() {
            *keys = true;
        }
    }
    if let Some((name, false)) = current {
        empty.push(name);
    }
    empty
}

#[test]
fn an_empty_local_section_is_not_a_remote() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["remote", "add", "origin", "https://example.com/o.git"]);
    // An empty section as found in the maintainer's repository (removing
    // keys one by one can leave one behind, depending on the Git version).
    let config = repo.path().join(".git").join("config");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("[remote \"github-fc\"]\n");
    std::fs::write(&config, text).unwrap();
    assert_eq!(empty_remote_sections(&repo), ["github-fc"]);
    assert_eq!(repo.git(&["remote"]), "origin", "Git's own view");

    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(backend.remotes().unwrap(), ["origin"]);
    assert!(backend.external_remotes().unwrap().is_empty());

    // One key makes it a remote again.
    repo.git(&[
        "config",
        "remote.github-fc.pushurl",
        "https://example.com/p.git",
    ]);
    backend.reload();
    assert_eq!(backend.remotes().unwrap(), ["github-fc", "origin"]);
}

#[test]
fn a_section_with_one_key_is_a_remote() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["config", "remote.mirror.url", "https://example.com/m.git"]);
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(backend.remotes().unwrap(), ["mirror"]);
}

#[test]
fn remote_operations_leave_no_empty_section() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    let cli = CliBackend::new(repo.path());
    let check = |step: &str| {
        assert!(
            empty_remote_sections(&repo).is_empty(),
            "after {step}: {:?}",
            empty_remote_sections(&repo)
        );
    };

    cli.add_remote("origin", "https://example.com/o.git")
        .unwrap();
    check("add");
    cli.set_remote_url("origin", "https://example.com/new.git")
        .unwrap();
    check("edit URL");
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    cli.set_upstream("main", "origin/main").unwrap();
    check("set upstream");
    cli.unset_upstream("main").unwrap();
    check("unset upstream");
    cli.rename_remote("origin", "upstream").unwrap();
    check("rename");
    cli.set_upstream("main", "upstream/main").unwrap();
    cli.remove_remote("upstream").unwrap();
    check("remove");

    let backend = GixBackend::discover(repo.path()).unwrap();
    assert!(backend.remotes().unwrap().is_empty());
}
