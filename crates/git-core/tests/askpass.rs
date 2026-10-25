// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// The fake askpass programs are shell scripts.
#![cfg(unix)]

mod support;

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use git_core::{CliBackend, Error, GitWriter, RunOptions};
use support::TestRepo;

/// Writes an executable askpass script that logs its prompt (and whether
/// the askpass marker is set) and answers with `body`.
fn fake_askpass(dir: &Path, body: &str) -> (PathBuf, PathBuf) {
    let log = dir.join("askpass.log");
    let script = dir.join("askpass.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$GITLOOM_ASKPASS|$1\" >> '{}'\n{body}\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (script, log)
}

/// `git credential fill` asks for username and password through askpass
/// when no credential helper answers (no network needed).
fn credential_fill(cli: &CliBackend) -> git_core::Result<String> {
    cli.run(
        &["-c", "credential.helper=", "credential", "fill"],
        RunOptions {
            stdin: Some(b"protocol=https\nhost=example.com\n\n"),
            read_only: true,
        },
    )
    .map(|output| output.stdout_text())
}

#[test]
fn prompts_go_to_the_askpass_program() {
    let repo = TestRepo::new();
    let (script, log) = fake_askpass(repo.path(), "echo answer");
    let cli = CliBackend::new(repo.path()).with_askpass(&script);

    let output = credential_fill(&cli).unwrap();
    assert!(output.contains("username=answer"), "{output}");
    assert!(output.contains("password=answer"), "{output}");
    let prompts = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        prompts.lines().collect::<Vec<_>>(),
        [
            "1|Username for 'https://example.com': ",
            "1|Password for 'https://answer@example.com': "
        ]
    );
}

#[test]
fn cancelled_prompt_fails_the_command() {
    let repo = TestRepo::new();
    let (script, _) = fake_askpass(repo.path(), "exit 1");
    let cli = CliBackend::new(repo.path()).with_askpass(&script);
    match credential_fill(&cli) {
        Err(Error::Command { stderr, .. }) => {
            assert!(stderr.contains("could not read Username"), "{stderr}")
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn without_askpass_git_never_prompts() {
    let repo = TestRepo::new();
    // GIT_TERMINAL_PROMPT=0 makes the prompt fail instead of hanging.
    assert!(credential_fill(&CliBackend::new(repo.path())).is_err());
}
