//! SSH key per repository (M8-05): a fake `ssh` in `PATH` records how Git
//! runs it for a fetch through Gitloom's network path. Its own test binary
//! (it changes `PATH` for the process); Linux and macOS only, Windows is
//! covered by the release checklist.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(unix)]

mod support;

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use git_core::ssh::{set_ssh_key, unset_ssh_command};
use git_core::{CancelToken, CliBackend, GitBackend, GitWriter, GixBackend, SshSetting};
use support::TestRepo;

/// Fetches `origin` through the network path; the fake ssh fails it.
fn fetch(cli: &CliBackend) {
    let result = cli.run_network(&["fetch", "origin"], &CancelToken::new(), &mut |_| {});
    assert!(result.is_err(), "the fake ssh refuses every connection");
}

fn recorded(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    std::fs::write(log, "").unwrap();
    text
}

#[test]
fn a_chosen_key_is_used_by_fetch() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    repo.git(&["remote", "add", "origin", "ssh://example.invalid/repo"]);
    let dir = repo.path().join("..").join("fake-ssh");
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("args.log");
    let ssh = dir.join("ssh");
    std::fs::write(
        &ssh,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 255\n", log.display()),
    )
    .unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{path}", dir.display()));
    std::env::remove_var("GIT_SSH");
    std::env::remove_var("GIT_SSH_COMMAND");
    std::env::set_var("GIT_SSH_VARIANT", "ssh");
    let key = repo.path().join("..").join("keys dir").join("id_work");
    let cli = CliBackend::new(repo.path());
    let backend = GixBackend::discover(repo.path()).unwrap();

    // System default: Git runs plain ssh, no key.
    fetch(&cli);
    let default = recorded(&log);
    assert!(default.contains("example.invalid"), "{default}");
    assert!(
        !default.contains("-i ") && !default.contains("IdentitiesOnly"),
        "{default}"
    );
    assert_eq!(
        backend.ssh_config().unwrap().setting,
        SshSetting::SystemDefault
    );

    // A key: `-i <key>` and IdentitiesOnly=yes, also with a space in the path.
    set_ssh_key(&cli, &key).unwrap();
    let config = backend.ssh_config().unwrap();
    assert_eq!(config.setting, SshSetting::Key(key.clone()));
    assert_eq!(config.origin, Some(git_core::ConfigOrigin::Local));
    fetch(&cli);
    let keyed = recorded(&log);
    assert!(keyed.contains(&format!("-i {}", key.display())), "{keyed}");
    assert!(keyed.contains("IdentitiesOnly=yes"), "{keyed}");

    // Back to the system default; unsetting twice is fine.
    unset_ssh_command(&cli).unwrap();
    unset_ssh_command(&cli).unwrap();
    assert_eq!(
        backend.ssh_config().unwrap().setting,
        SshSetting::SystemDefault
    );
    fetch(&cli);
    assert!(!recorded(&log).contains("-i "));
    let config = std::fs::read_to_string(repo.path().join(".git").join("config")).unwrap();
    assert!(
        config.contains("[core]") && !config.contains("sshCommand"),
        "{config}"
    );

    // A custom command written elsewhere is read as such.
    repo.git(&["config", "core.sshCommand", "ssh -v"]);
    assert_eq!(
        backend.ssh_config().unwrap().setting,
        SshSetting::Custom("ssh -v".into())
    );
    let _ = cli.run(
        &["config", "--unset", "core.sshCommand"],
        Default::default(),
    );
}
