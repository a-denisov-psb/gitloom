//! Git flow configuration (M7-06) against real repositories.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{flow, CliBackend, FlowConfig, GitBackend, GixBackend};
use support::TestRepo;

/// What `git flow init -d` (AVH edition) writes.
fn init_like_the_cli(repo: &TestRepo) {
    for (key, value) in [
        ("gitflow.branch.master", "main"),
        ("gitflow.branch.develop", "develop"),
        ("gitflow.prefix.feature", "feature/"),
        ("gitflow.prefix.bugfix", "bugfix/"),
        ("gitflow.prefix.release", "release/"),
        ("gitflow.prefix.hotfix", "hotfix/"),
        ("gitflow.prefix.support", "support/"),
        ("gitflow.prefix.versiontag", ""),
        ("gitflow.path.hooks", ".git/hooks"),
    ] {
        repo.git(&["config", key, value]);
    }
}

fn config_value(repo: &TestRepo, key: &str) -> String {
    repo.git(&["config", "--get", key])
}

#[test]
fn a_repository_initialized_by_the_cli_is_recognized() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.branch("develop");
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(backend.flow_config().unwrap(), None, "not initialized");
    init_like_the_cli(&repo);
    let config = backend.flow_config().unwrap().unwrap();
    assert_eq!(config, FlowConfig::defaults(&["main"]));
}

#[test]
fn init_creates_missing_branches_and_writes_the_cli_keys() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    let head = repo.head();
    let writer = CliBackend::new(repo.path());
    let config = FlowConfig {
        version_tag: "v".into(),
        ..FlowConfig::defaults(&["main"])
    };
    flow::init(&writer, &config).unwrap();

    // develop from the production tip; HEAD stays on main.
    assert_eq!(repo.git(&["rev-parse", "develop"]), head);
    assert_eq!(repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    // The keys the git-flow CLI reads.
    assert_eq!(config_value(&repo, "gitflow.branch.master"), "main");
    assert_eq!(config_value(&repo, "gitflow.branch.develop"), "develop");
    assert_eq!(config_value(&repo, "gitflow.prefix.feature"), "feature/");
    assert_eq!(config_value(&repo, "gitflow.prefix.release"), "release/");
    assert_eq!(config_value(&repo, "gitflow.prefix.hotfix"), "hotfix/");
    assert_eq!(config_value(&repo, "gitflow.prefix.versiontag"), "v");
    let backend = GixBackend::discover(repo.path()).unwrap();
    assert_eq!(backend.flow_config().unwrap(), Some(config));
}

#[test]
fn init_creates_a_missing_production_branch_from_head() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["branch", "-m", "work"]);
    let writer = CliBackend::new(repo.path());
    let config = FlowConfig::defaults(&["work"]);
    assert_eq!(config.production, "master");
    flow::init(&writer, &config).unwrap();
    let head = repo.head();
    assert_eq!(repo.git(&["rev-parse", "master"]), head);
    assert_eq!(repo.git(&["rev-parse", "develop"]), head);
}

#[test]
fn init_refuses_a_repository_without_commits() {
    let repo = TestRepo::new();
    let writer = CliBackend::new(repo.path());
    let error = flow::init(&writer, &FlowConfig::defaults(&[])).unwrap_err();
    assert!(error.to_string().contains("at least one commit"), "{error}");
}

#[test]
fn write_keeps_other_keys_and_remove_drops_only_the_config() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.branch("develop");
    init_like_the_cli(&repo);
    let writer = CliBackend::new(repo.path());
    let edited = FlowConfig {
        feature: "feat/".into(),
        ..FlowConfig::defaults(&["main"])
    };
    flow::write(&writer, &edited).unwrap();
    assert_eq!(config_value(&repo, "gitflow.prefix.feature"), "feat/");
    assert_eq!(
        config_value(&repo, "gitflow.prefix.bugfix"),
        "bugfix/",
        "kept"
    );

    let backend = GixBackend::discover(repo.path()).unwrap();
    flow::remove(&writer).unwrap();
    assert_eq!(backend.flow_config().unwrap(), None);
    assert!(!repo.try_git(&["config", "--get-regexp", "^gitflow\\."]));
    assert!(
        repo.try_git(&["rev-parse", "--verify", "develop"]),
        "branches stay"
    );
    // Removing again is harmless.
    flow::remove(&writer).unwrap();
}

#[test]
fn write_rejects_an_invalid_configuration() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    let writer = CliBackend::new(repo.path());
    let same = FlowConfig {
        development: "main".into(),
        ..FlowConfig::defaults(&["main"])
    };
    assert!(flow::write(&writer, &same).is_err());
    assert!(!repo.try_git(&["config", "--get-regexp", "^gitflow\\."]));
}
