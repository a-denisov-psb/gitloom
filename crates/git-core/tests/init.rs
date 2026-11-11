//! Start a local repository (M10-03, ADR-045).

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use git_core::init::{init_with, InitOptions, InitOutcome};
use git_core::{CliBackend, GitBackend, GitWriter, GixBackend, Head};

fn git(target: &Path, identity: bool) -> CliBackend {
    let (name, email) = if identity {
        ("Ana Lyst", "ana@example.com")
    } else {
        ("", "")
    };
    CliBackend::new(target)
        .with_config("user.name", name)
        .with_config("user.email", email)
        .with_config("commit.gpgSign", "false")
}

fn options(readme: bool, gitignore: Option<&str>) -> InitOptions {
    InitOptions {
        branch: "trunk".into(),
        readme,
        gitignore: gitignore.map(str::to_owned),
    }
}

fn run(args: &[&str], dir: &Path) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn init_with_files_commits_them_on_the_chosen_branch() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("project");
    let outcome = init_with(
        git(&target, true),
        &target,
        "project",
        &options(true, Some("Rust")),
    )
    .unwrap();
    assert_eq!(outcome, InitOutcome::Committed);
    assert_eq!(run(&["branch", "--show-current"], &target), "trunk");
    assert_eq!(
        run(&["log", "--format=%s %an"], &target),
        "Initial commit Ana Lyst"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("README.md")).unwrap(),
        "# project\n"
    );
    assert!(std::fs::read_to_string(target.join(".gitignore"))
        .unwrap()
        .contains("target"));
    assert_eq!(run(&["status", "--porcelain"], &target), "");
}

#[test]
fn init_without_identity_leaves_the_files_uncommitted() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("project");
    let outcome = init_with(
        git(&target, false),
        &target,
        "project",
        &options(true, None),
    )
    .unwrap();
    assert_eq!(outcome, InitOutcome::NoIdentity);
    assert_eq!(run(&["status", "--porcelain"], &target), "?? README.md");
}

/// An empty repository (unborn branch) loads and can get its first commit.
#[test]
fn empty_repository_loads_and_commits() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("empty");
    let outcome = init_with(git(&target, true), &target, "empty", &options(false, None)).unwrap();
    assert_eq!(outcome, InitOutcome::Empty);

    let backend = GixBackend::discover(&target).unwrap();
    assert!(backend.log(100).unwrap().commits.is_empty());
    assert!(matches!(backend.head().unwrap(), Head::Unborn { name } if name == "trunk"));
    assert!(backend.working_tree().unwrap().is_empty());

    std::fs::write(target.join("a.txt"), "a\n").unwrap();
    let cli = git(&target, true);
    cli.stage(&["a.txt".into()]).unwrap();
    cli.commit("First", false).unwrap();
    let backend = GixBackend::discover(&target).unwrap();
    assert_eq!(backend.log(100).unwrap().commits.len(), 1);
}

#[test]
fn non_empty_target_and_bad_branch_are_refused() {
    let parent = tempfile::tempdir().unwrap();
    let busy = parent.path().join("busy");
    std::fs::create_dir(&busy).unwrap();
    std::fs::write(busy.join("x"), "x").unwrap();
    let error = init_with(git(&busy, true), &busy, "busy", &options(true, None)).unwrap_err();
    assert!(error.to_string().contains("not empty"), "{error}");

    let target = parent.path().join("new");
    let mut bad = options(false, None);
    bad.branch = "has space".into();
    assert!(init_with(git(&target, true), &target, "new", &bad).is_err());
    assert!(!target.exists());
}
