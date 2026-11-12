//! Undo and redo of pull, remotes, upstream and submodule update (M12-08)
//! against local repositories, including refused cases.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::path::{Path, PathBuf};

use git_core::remote_undo::{
    add_remote_recorded, remove_remote_recorded, set_url_recorded, switch_remote_change,
    upstream_recorded, RemoteChange,
};
use git_core::submodule::{heads, switch_update, update_record, SubmoduleState};
use git_core::{
    record_branch_move, record_pull, redo_history, undo_history, CliBackend, GitWriter as _,
    RunOptions,
};
use support::TestRepo;

fn url(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A "server" repository and a clone of it (in the second repository's
/// temporary folder) with an identity for merge commits.
fn server_and_clone() -> (TestRepo, TestRepo, PathBuf) {
    let server = TestRepo::new();
    server.commit("a.txt", "a\n", "base");
    server.branch("topic");
    let holder = TestRepo::new();
    let clone = holder.path().join("clone");
    holder.git(&["clone", "--quiet", &url(server.path()), &url(&clone)]);
    holder.git_in(&clone, &["config", "user.name", "Test Author"]);
    holder.git_in(&clone, &["config", "user.email", "author@example.com"]);
    (server, holder, clone)
}

fn pull(git: &CliBackend, mode: &str) -> git_core::Result<git_core::GitOutput> {
    git.run(&["pull", mode, "--quiet"], RunOptions::default())
}

fn head(holder: &TestRepo, clone: &Path) -> String {
    holder.git_in(clone, &["rev-parse", "HEAD"])
}

#[test]
fn pull_fast_forward_merge_and_rebase_undo_and_redo() {
    for mode in ["--ff-only", "--ff", "--rebase"] {
        let (server, holder, clone) = server_and_clone();
        server.commit("b.txt", "b\n", "server change");
        let git = CliBackend::new(&clone);
        if mode != "--ff-only" {
            std::fs::write(clone.join("c.txt"), "c\n").unwrap();
            holder.git_in(&clone, &["add", "c.txt"]);
            holder.git_in(&clone, &["commit", "--quiet", "-m", "local change"]);
        }
        let before = head(&holder, &clone);
        let (_, change) = record_pull(&git, "Pull", || pull(&git, mode)).unwrap();
        let change = change.expect(mode);
        let after = head(&holder, &clone);
        assert_ne!(before, after, "{mode}");
        let fetched = holder.git_in(&clone, &["rev-parse", "origin/main"]);
        let undone = undo_history(&git, &clone, &change).unwrap();
        assert_eq!(head(&holder, &clone), before, "{mode}");
        // The fetched remote-tracking branch stays.
        assert_eq!(
            holder.git_in(&clone, &["rev-parse", "origin/main"]),
            fetched
        );
        redo_history(&git, &clone, &undone).unwrap();
        assert_eq!(head(&holder, &clone), after, "{mode}");
        // A new commit after the pull: undo refuses.
        std::fs::write(clone.join("d.txt"), "d\n").unwrap();
        holder.git_in(&clone, &["add", "d.txt"]);
        holder.git_in(&clone, &["commit", "--quiet", "-m", "later"]);
        assert!(undo_history(&git, &clone, &change).is_err(), "{mode}");
    }
}

#[test]
fn pull_of_a_branch_that_is_not_checked_out() {
    let (server, holder, clone) = server_and_clone();
    server.checkout("topic");
    server.commit("t.txt", "t\n", "topic change");
    holder.git_in(
        &clone,
        &["branch", "--quiet", "--track", "topic", "origin/topic"],
    );
    let git = CliBackend::new(&clone);
    let before = holder.git_in(&clone, &["rev-parse", "topic"]);
    let args = git_core::fast_forward_args("origin", "topic", "topic");
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (_, change) = record_branch_move(&git, "topic", "Pull topic", || {
        git.run(&args, RunOptions::default())
    })
    .unwrap();
    let change = change.unwrap();
    let after = holder.git_in(&clone, &["rev-parse", "topic"]);
    undo_history(&git, &clone, &change).unwrap();
    assert_eq!(holder.git_in(&clone, &["rev-parse", "topic"]), before);
    redo_history(&git, &clone, &change).unwrap();
    assert_eq!(holder.git_in(&clone, &["rev-parse", "topic"]), after);
}

/// Repository config (sorted: `git remote rename` reorders keys) and
/// remote-tracking refs, comparable.
fn remote_state(repo: &TestRepo) -> (String, String) {
    let config = repo.git(&["config", "--local", "--list"]);
    let mut lines: Vec<&str> = config.lines().collect();
    lines.sort_unstable();
    (
        lines.join("\n"),
        repo.git(&[
            "for-each-ref",
            "--format=%(refname) %(objectname) %(symref)",
            "refs/remotes",
        ]),
    )
}

fn with_upstream_remote() -> TestRepo {
    let repo = TestRepo::new();
    let head = repo.commit("a.txt", "a\n", "base");
    repo.fake_upstream("upstream", "main", &head);
    repo.git(&["config", "remote.upstream.pushurl", "git@example.org:x.git"]);
    repo.git(&[
        "symbolic-ref",
        "refs/remotes/upstream/HEAD",
        "refs/remotes/upstream/main",
    ]);
    repo
}

#[test]
fn remove_remote_restores_config_tracking_refs_and_upstreams() {
    let repo = with_upstream_remote();
    let git = CliBackend::new(repo.path());
    let before = remote_state(&repo);
    let change = remove_remote_recorded(&git, "upstream").unwrap();
    let after = remote_state(&repo);
    assert!(!after.0.contains("upstream") && after.1.is_empty());
    switch_remote_change(&git, &change, true).unwrap();
    assert_eq!(remote_state(&repo), before);
    switch_remote_change(&git, &change, false).unwrap();
    assert_eq!(remote_state(&repo), after);
    // A remote with that name was added since: undo refuses.
    repo.git(&["remote", "add", "upstream", "https://example.org/other.git"]);
    let outside = remote_state(&repo);
    assert!(switch_remote_change(&git, &change, true).is_err());
    assert_eq!(remote_state(&repo), outside);
}

#[test]
fn add_rename_and_url_changes() {
    let repo = with_upstream_remote();
    let git = CliBackend::new(repo.path());
    let start = remote_state(&repo);

    let add = add_remote_recorded(&git, "fork", "https://example.org/fork.git").unwrap();
    switch_remote_change(&git, &add, true).unwrap();
    assert_eq!(remote_state(&repo), start);
    switch_remote_change(&git, &add, false).unwrap();
    repo.git(&["remote", "set-url", "fork", "https://example.org/moved.git"]);
    assert!(
        switch_remote_change(&git, &add, true).is_err(),
        "URL changed since"
    );
    repo.git(&["remote", "remove", "fork"]);

    git.rename_remote("upstream", "up").unwrap();
    let rename = RemoteChange::Rename {
        old: "upstream".into(),
        new: "up".into(),
    };
    let renamed = remote_state(&repo);
    switch_remote_change(&git, &rename, true).unwrap();
    assert_eq!(remote_state(&repo), start, "refs and upstreams follow back");
    switch_remote_change(&git, &rename, false).unwrap();
    assert_eq!(remote_state(&repo), renamed);
    repo.git(&["remote", "add", "upstream", "https://example.org/x.git"]);
    assert!(switch_remote_change(&git, &rename, true).is_err());
    repo.git(&["remote", "remove", "upstream"]);
    switch_remote_change(&git, &rename, true).unwrap();

    let set = set_url_recorded(&git, "upstream", "https://example.org/new.git").unwrap();
    assert_eq!(
        repo.git(&["remote", "get-url", "upstream"]),
        "https://example.org/new.git"
    );
    switch_remote_change(&git, &set, true).unwrap();
    assert_eq!(remote_state(&repo), start);
    switch_remote_change(&git, &set, false).unwrap();
    repo.git(&[
        "remote",
        "set-url",
        "upstream",
        "https://example.org/third.git",
    ]);
    assert!(switch_remote_change(&git, &set, true).is_err());
}

#[test]
fn upstream_set_and_unset() {
    let repo = with_upstream_remote();
    let git = CliBackend::new(repo.path());
    let start = remote_state(&repo);
    let unset = upstream_recorded(&git, "main", None).unwrap();
    assert!(!remote_state(&repo).0.contains("branch.main.merge"));
    switch_remote_change(&git, &unset, true).unwrap();
    assert_eq!(remote_state(&repo), start);
    switch_remote_change(&git, &unset, false).unwrap();
    let set = upstream_recorded(&git, "main", Some("upstream/main")).unwrap();
    assert_eq!(remote_state(&repo), start);
    switch_remote_change(&git, &set, true).unwrap();
    assert!(!remote_state(&repo).0.contains("branch.main.remote"));
    // Changed outside: redo refuses.
    repo.git(&["config", "branch.main.remote", "upstream"]);
    repo.git(&["config", "branch.main.merge", "refs/heads/other"]);
    assert!(switch_remote_change(&git, &set, false).is_err());
}

const ALLOW_FILE: [&str; 2] = ["-c", "protocol.file.allow=always"];

#[test]
fn submodule_update_undo_and_redo() {
    let lib = TestRepo::new();
    lib.commit("lib.txt", "one\n", "one");
    let top = TestRepo::new();
    top.commit("README.md", "top\n", "top");
    top.git(&[
        ALLOW_FILE[0],
        ALLOW_FILE[1],
        "submodule",
        "add",
        "--quiet",
        &url(lib.path()),
        "lib",
    ]);
    top.git(&["commit", "--quiet", "-m", "add lib"]);
    let git = CliBackend::new(top.path());
    let update = |paths: &[&str]| {
        let mut args = vec![
            ALLOW_FILE[0],
            ALLOW_FILE[1],
            "submodule",
            "update",
            "--init",
            "--recursive",
            "--quiet",
            "--",
        ];
        args.extend(paths);
        git.run(&args, RunOptions::default()).unwrap();
    };
    let state = || {
        git_core::submodule::submodules(&git, top.path())
            .unwrap()
            .into_iter()
            .map(|s| (s.commit, s.state))
            .collect::<Vec<_>>()
    };

    // Out of date (a newer commit checked out) → update → undo → redo.
    let lib_dir = top.path().join("lib");
    std::fs::write(lib_dir.join("lib.txt"), "two\n").unwrap();
    top.git_in(&lib_dir, &["commit", "--quiet", "-am", "two"]);
    assert_eq!(state()[0].1, SubmoduleState::OutOfDate);
    let out_of_date = state();
    let before = heads(&git, top.path()).unwrap();
    update(&["lib"]);
    let after = heads(&git, top.path()).unwrap();
    let record = update_record(&before, &after).unwrap();
    let updated = state();
    assert_eq!(updated[0].1, SubmoduleState::Current);
    switch_update(&git, top.path(), &record, true).unwrap();
    assert_eq!(state(), out_of_date);
    switch_update(&git, top.path(), &record, false).unwrap();
    assert_eq!(state(), updated);
    // Local changes in the submodule: undo refuses.
    std::fs::write(lib_dir.join("lib.txt"), "edited\n").unwrap();
    assert!(switch_update(&git, top.path(), &record, true).is_err());
    std::fs::write(lib_dir.join("lib.txt"), "one\n").unwrap();

    // Not initialized → update → undo de-initializes → redo initializes.
    top.git(&["submodule", "deinit", "--quiet", "--force", "--", "lib"]);
    assert_eq!(state()[0].1, SubmoduleState::NotInitialized);
    let before = heads(&git, top.path()).unwrap();
    update(&[]);
    let record = update_record(&before, &heads(&git, top.path()).unwrap()).unwrap();
    switch_update(&git, top.path(), &record, true).unwrap();
    assert_eq!(state()[0].1, SubmoduleState::NotInitialized);
    switch_update(&git, top.path(), &record, false).unwrap();
    assert_eq!(state()[0].1, SubmoduleState::Current);
}
