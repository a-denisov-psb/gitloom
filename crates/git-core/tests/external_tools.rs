// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    configured_tool, run_difftool, run_mergetool, tool_info, tool_shell_command, CliBackend,
    DiffTarget, StashPart, ToolKind, ToolSelection,
};
use support::TestRepo;

/// Configures a fake diff tool that appends the compared file contents
/// (and what it read from stdin) to `calls.txt`, and a fake merge tool
/// that resolves the conflict with "theirs".
fn repo_with_fake_tools() -> TestRepo {
    let repo = TestRepo::new();
    let log = repo.path().join("..").join("calls.txt");
    let log = log.to_string_lossy().replace('\\', "/");
    repo.git(&[
        "config",
        "difftool.fake.cmd",
        &format!(
            "cat \"$LOCAL\" \"$REMOTE\" | tr '\\n' ' ' >> '{log}'; echo >> '{log}'; cat >> '{log}'"
        ),
    ]);
    repo.git(&[
        "config",
        "mergetool.fakemerge.cmd",
        "cat \"$REMOTE\" > \"$MERGED\"; cat > /dev/null",
    ]);
    repo.git(&["config", "mergetool.fakemerge.trustExitCode", "true"]);
    repo.git(&["config", "mergetool.keepBackup", "false"]);
    repo
}

fn fake() -> ToolSelection {
    ToolSelection::Named("fake".into())
}

fn calls(repo: &TestRepo) -> String {
    std::fs::read_to_string(repo.path().join("..").join("calls.txt")).unwrap_or_default()
}

#[test]
fn difftool_for_commit_unstaged_and_staged() {
    let repo = repo_with_fake_tools();
    repo.commit("a.txt", "one\n", "first");
    let second = repo.commit("a.txt", "two\n", "second");
    let parent = repo.parents(&second)[0].clone();
    let cli = CliBackend::new(repo.path());

    let commit = DiffTarget::Commit {
        id: second,
        parent: Some(parent),
    };
    run_difftool(&cli, &commit, &fake(), "a.txt", None).unwrap();
    assert_eq!(calls(&repo), "one two \n");

    repo.write("a.txt", "three\n");
    run_difftool(&cli, &DiffTarget::Unstaged, &fake(), "a.txt", None).unwrap();
    assert!(calls(&repo).ends_with("two three \n"), "{}", calls(&repo));

    repo.git(&["add", "a.txt"]);
    run_difftool(&cli, &DiffTarget::Staged, &fake(), "a.txt", None).unwrap();
    assert!(calls(&repo).ends_with("two three \n"));
    assert_eq!(
        calls(&repo).lines().count(),
        3,
        "stdin was closed: nothing read"
    );
}

#[test]
fn difftool_for_a_root_commit_compares_with_the_empty_tree() {
    let repo = repo_with_fake_tools();
    let root = repo.commit("a.txt", "one\n", "first");
    let cli = CliBackend::new(repo.path());
    let target = DiffTarget::Commit {
        id: root,
        parent: None,
    };
    run_difftool(&cli, &target, &fake(), "a.txt", None).unwrap();
    assert_eq!(calls(&repo).trim(), "one");
}

#[test]
fn mergetool_resolves_the_conflict() {
    let repo = repo_with_fake_tools();
    repo.commit("c.txt", "base\n", "base");
    repo.branch("side");
    repo.checkout("side");
    repo.commit("c.txt", "theirs\n", "side");
    repo.checkout("main");
    repo.commit("c.txt", "ours\n", "main");
    assert!(!repo.try_git(&["merge", "side"]));
    let cli = CliBackend::new(repo.path());
    run_mergetool(
        &cli,
        &ToolSelection::Named("fakemerge".into()),
        "c.txt",
        false,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("c.txt")).unwrap(),
        "theirs\n"
    );
    // Resolved and staged by mergetool.
    assert!(repo
        .git(&["diff", "--name-only", "--diff-filter=U"])
        .is_empty());
}

/// A command template (fix after M15, ADR-055): the same fake diff tool,
/// given per call, with a program path containing a space; nothing is
/// written to the repository's config.
fn custom_diff(repo: &TestRepo) -> ToolSelection {
    let dir = repo.path().join("..").join("my tools");
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("fake diff.sh");
    let log = repo.path().join("..").join("calls.txt");
    let log = log.to_string_lossy().replace('\\', "/");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ncat \"$1\" \"$2\" | tr '\\n' ' ' >> '{log}'; echo >> '{log}'\n"),
    )
    .unwrap();
    let program = script.to_string_lossy().replace('\\', "/");
    let words: Vec<String> = ["sh", &program, "{local}", "{remote}"]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
    ToolSelection::Command(tool_shell_command(&words, false))
}

#[test]
fn custom_difftool_for_commit_unstaged_and_staged() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "one\n", "first");
    let second = repo.commit("a.txt", "two\n", "second");
    let parent = repo.parents(&second)[0].clone();
    let cli = CliBackend::new(repo.path());
    let tool = custom_diff(&repo);
    let commit = DiffTarget::Commit {
        id: second,
        parent: Some(parent),
    };
    run_difftool(&cli, &commit, &tool, "a.txt", None).unwrap();
    assert_eq!(calls(&repo), "one two \n");
    repo.write("a.txt", "three\n");
    run_difftool(&cli, &DiffTarget::Unstaged, &tool, "a.txt", None).unwrap();
    assert!(calls(&repo).ends_with("two three \n"), "{}", calls(&repo));
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "four\n");
    run_difftool(&cli, &DiffTarget::Staged, &tool, "a.txt", None).unwrap();
    assert!(calls(&repo).ends_with("two three \n"), "{}", calls(&repo));
    assert_eq!(calls(&repo).lines().count(), 3);
    assert!(!repo.try_git(&["config", "--get-regexp", "gitloom-custom"]));
}

/// A command tool that copies LOCAL and REMOTE to `sides/local` and
/// `sides/remote` next to the repository.
fn side_copier(repo: &TestRepo) -> (ToolSelection, std::path::PathBuf) {
    let dir = repo.path().join("..").join("sides");
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("copy.sh");
    let out = dir.to_string_lossy().replace('\\', "/");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ncat \"$1\" > '{out}/local'; cat \"$2\" > '{out}/remote'\n"),
    )
    .unwrap();
    let program = script.to_string_lossy().replace('\\', "/");
    let words: Vec<String> = ["sh", &program, "{local}", "{remote}"]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
    (
        ToolSelection::Command(tool_shell_command(&words, false)),
        dir,
    )
}

/// Finding #57: each part of a stash opens with the sides of the internal
/// diff.
#[test]
fn difftool_for_the_parts_of_a_stash() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "head\n", "first");
    repo.write("a.txt", "index\n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", "worktree\n");
    repo.write("new.txt", "untracked\n");
    repo.git(&["stash", "push", "--include-untracked"]);
    let id = repo.git(&["rev-parse", "stash@{0}"]).trim().to_owned();
    let cli = CliBackend::new(repo.path());
    let (tool, dir) = side_copier(&repo);
    let sides = |part, path: &str| {
        let target = DiffTarget::Stash {
            id: id.clone(),
            part,
        };
        run_difftool(&cli, &target, &tool, path, None).unwrap();
        let read = |name| std::fs::read_to_string(dir.join(name)).unwrap();
        (read("local"), read("remote"))
    };
    assert_eq!(
        sides(StashPart::Staged, "a.txt"),
        ("head\n".into(), "index\n".into())
    );
    assert_eq!(
        sides(StashPart::Unstaged, "a.txt"),
        ("index\n".into(), "worktree\n".into())
    );
    assert_eq!(
        sides(StashPart::Untracked, "new.txt"),
        (String::new(), "untracked\n".into())
    );
}

fn conflicted() -> TestRepo {
    let repo = TestRepo::new();
    repo.commit("c.txt", "base\n", "base");
    repo.branch("side");
    repo.checkout("side");
    repo.commit("c.txt", "theirs\n", "side");
    repo.checkout("main");
    repo.commit("c.txt", "ours\n", "main");
    assert!(!repo.try_git(&["merge", "side"]));
    repo
}

fn custom_merge(script: &str) -> ToolSelection {
    let words: Vec<String> = [
        "sh", "-c", script, "merge", "{local}", "{remote}", "{base}", "{merged}",
    ]
    .iter()
    .map(|w| (*w).to_owned())
    .collect();
    ToolSelection::Command(tool_shell_command(&words, false))
}

fn unresolved(repo: &TestRepo) -> String {
    repo.git(&["diff", "--name-only", "--diff-filter=U"])
}

#[test]
fn custom_mergetool_resolves_when_saved() {
    let repo = conflicted();
    let cli = CliBackend::new(repo.path());
    // Writes "theirs" + base into the result and exits 1: the exit code is
    // not trusted, the saved file counts. Git compares modification times
    // with `test -nt`, which some shells (macOS) do in whole seconds: the
    // tool saves a second after Git's backup, as a person would.
    let tool = custom_merge(r#"sleep 1; cat "$2" "$3" > "$4"; exit 1"#);
    run_mergetool(&cli, &tool, "c.txt", true).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("c.txt")).unwrap(),
        "theirs\nbase\n"
    );
    assert!(unresolved(&repo).is_empty());
    assert!(!repo.path().join("c.txt.orig").exists());
}

#[test]
fn custom_mergetool_closed_without_saving_never_waits() {
    let repo = conflicted();
    let cli = CliBackend::new(repo.path());
    // Exits at once without touching the result: Git would ask "Was the
    // merge successful?"; stdin is closed, so it stays conflicted.
    let tool = custom_merge("exit 0");
    let start = std::time::Instant::now();
    assert!(run_mergetool(&cli, &tool, "c.txt", false).is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
    assert_eq!(unresolved(&repo).trim(), "c.txt");
}

#[test]
fn tool_help_works_outside_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let cli = CliBackend::new(dir.path());
    // Git lists its tools without a repository (Preferences → External
    // tools works before any repository is open).
    assert!(tool_info(&cli, ToolKind::Diff).is_ok());
    assert!(tool_info(&cli, ToolKind::Merge).is_ok());
}

#[test]
fn tool_info_lists_configured_and_user_defined_tools() {
    let repo = repo_with_fake_tools();
    repo.commit("a.txt", "one\n", "first");
    let cli = CliBackend::new(repo.path());
    assert_eq!(configured_tool(&cli, ToolKind::Diff).unwrap(), None);
    let info = tool_info(&cli, ToolKind::Diff).unwrap();
    assert!(info.available.contains(&"fake".to_owned()), "{info:?}");
    // diff.tool falls back to merge.tool, like Git.
    repo.git(&["config", "merge.tool", "fakemerge"]);
    assert_eq!(
        configured_tool(&cli, ToolKind::Diff).unwrap().as_deref(),
        Some("fakemerge")
    );
    repo.git(&["config", "diff.tool", "fake"]);
    assert_eq!(
        configured_tool(&cli, ToolKind::Diff).unwrap().as_deref(),
        Some("fake")
    );
    assert_eq!(
        configured_tool(&cli, ToolKind::Merge).unwrap().as_deref(),
        Some("fakemerge")
    );
}

/// The repository's external editor is written to and removed from its
/// config, and read by the gix backend (M10-11).
#[test]
fn repository_editor_round_trip() {
    use git_core::{GitBackend, GixBackend};
    let repo = support::TestRepo::new();
    let cli = CliBackend::new(repo.path());
    git_core::set_repository_editor(&cli, Some("zed {path}")).unwrap();
    assert_eq!(
        GixBackend::discover(repo.path())
            .unwrap()
            .external_editor()
            .unwrap()
            .as_deref(),
        Some("zed {path}")
    );
    git_core::set_repository_editor(&cli, None).unwrap();
    git_core::set_repository_editor(&cli, None).unwrap();
    assert_eq!(
        GixBackend::discover(repo.path())
            .unwrap()
            .external_editor()
            .unwrap(),
        None
    );
}
