// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    configured_tool, run_difftool, run_mergetool, tool_info, CliBackend, DiffTarget, ToolKind,
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
    run_difftool(&cli, &commit, Some("fake"), "a.txt", None).unwrap();
    assert_eq!(calls(&repo), "one two \n");

    repo.write("a.txt", "three\n");
    run_difftool(&cli, &DiffTarget::Unstaged, Some("fake"), "a.txt", None).unwrap();
    assert!(calls(&repo).ends_with("two three \n"), "{}", calls(&repo));

    repo.git(&["add", "a.txt"]);
    run_difftool(&cli, &DiffTarget::Staged, Some("fake"), "a.txt", None).unwrap();
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
    run_difftool(&cli, &target, Some("fake"), "a.txt", None).unwrap();
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
    run_mergetool(&cli, Some("fakemerge"), "c.txt", false).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("c.txt")).unwrap(),
        "theirs\n"
    );
    // Resolved and staged by mergetool.
    assert!(repo
        .git(&["diff", "--name-only", "--diff-filter=U"])
        .is_empty());
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
