// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{detect_git, CliBackend, Error, GitAvailability, GitWriter, RunOptions};
use support::TestRepo;

fn backend(repo: &TestRepo) -> CliBackend {
    CliBackend::new(repo.path())
}

#[test]
fn detects_installed_git() {
    assert!(
        matches!(detect_git(), GitAvailability::Available(_)),
        "tests need git >= 2.34"
    );
}

#[test]
fn runs_in_repository_root() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "First");
    let output = backend(&repo)
        .run(&["rev-parse", "--show-toplevel"], RunOptions::default())
        .unwrap();
    let top = std::path::PathBuf::from(output.stdout_text().trim());
    assert_eq!(
        top.canonicalize().unwrap(),
        repo.path().canonicalize().unwrap()
    );
}

#[test]
fn passes_stdin() {
    let repo = TestRepo::new();
    let output = backend(&repo)
        .run(
            &["hash-object", "-w", "--stdin"],
            RunOptions {
                stdin: Some(b"hello\n"),
                read_only: false,
            },
        )
        .unwrap();
    let id = output.stdout_text().trim().to_owned();
    assert_eq!(id, "ce013625030ba8dba906f756967f9e9ca394464a");
    assert_eq!(repo.git(&["cat-file", "blob", &id]), "hello");
}

#[test]
fn large_stdin_does_not_deadlock() {
    let repo = TestRepo::new();
    let data = vec![b'x'; 4 * 1024 * 1024];
    let output = backend(&repo)
        .run(
            &["hash-object", "--stdin"],
            RunOptions {
                stdin: Some(&data),
                read_only: true,
            },
        )
        .unwrap();
    assert_eq!(output.stdout_text().trim().len(), 40);
}

#[test]
fn failures_carry_command_exit_code_and_stderr() {
    let repo = TestRepo::new();
    let error = backend(&repo)
        .run(
            &["rev-parse", "--verify", "no-such-ref"],
            RunOptions::default(),
        )
        .unwrap_err();
    match &error {
        Error::Command {
            command,
            code,
            stderr,
        } => {
            assert_eq!(command, "git rev-parse --verify no-such-ref");
            assert_eq!(*code, Some(128));
            assert!(stderr.contains("Needed a single revision"), "{stderr}");
        }
        other => panic!("unexpected error {other:?}"),
    }
    assert!(error.to_string().contains("exit code 128"));
}

#[test]
fn never_prompts_or_opens_an_editor() {
    let repo = TestRepo::new();
    let cli = backend(&repo);
    let editor = cli
        .run(&["var", "GIT_EDITOR"], RunOptions::default())
        .unwrap();
    assert_eq!(editor.stdout_text().trim(), ":");
    // An editor-based commit would hang if an editor were opened; ":" leaves
    // the message empty, so Git aborts instead.
    repo.write("a.txt", "a\n");
    repo.git(&["add", "a.txt"]);
    let result = cli.run(&["commit"], RunOptions::default());
    assert!(matches!(result, Err(Error::Command { .. })));
}

#[test]
fn arguments_are_not_interpreted_by_a_shell() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "First");
    let weird = "file; echo pwned $(whoami).txt";
    repo.write(weird, "x\n");
    backend(&repo)
        .run(&["add", "--", weird], RunOptions::default())
        .unwrap();
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), weird);
}
