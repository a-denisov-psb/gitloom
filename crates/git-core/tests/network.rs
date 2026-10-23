// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{CancelToken, CliBackend, Error, GitWriter, Progress};
use support::TestRepo;

/// A repository with history and a fresh clone target pointing at it.
fn remote_and_local() -> (TestRepo, TestRepo) {
    let remote = TestRepo::new();
    for i in 0..20 {
        remote.commit(&format!("f{i}.txt"), &"content\n".repeat(200 + i), "Commit");
    }
    let local = TestRepo::new();
    local.git(&["remote", "add", "origin", remote.path().to_str().unwrap()]);
    (remote, local)
}

#[test]
fn fetch_reports_progress() {
    let (_remote, local) = remote_and_local();
    let mut updates: Vec<Progress> = Vec::new();
    CliBackend::new(local.path())
        .run_network(
            &["fetch", "--progress", "origin"],
            &CancelToken::new(),
            &mut |p| updates.push(p),
        )
        .unwrap();
    assert!(
        updates.iter().any(|p| p.percent == Some(100)),
        "{updates:?}"
    );
    // Updates arrive while the command runs, phase by phase.
    assert!(updates.len() > 2, "{updates:?}");
    assert!(!local.git(&["rev-parse", "origin/main"]).is_empty());
}

#[test]
fn failures_report_git_output_without_progress() {
    let local = TestRepo::new();
    let error = CliBackend::new(local.path())
        .run_network(
            &["fetch", "--progress", "/nonexistent/repository"],
            &CancelToken::new(),
            &mut |_| {},
        )
        .unwrap_err();
    match error {
        Error::Command { stderr, .. } => assert!(stderr.contains("fatal"), "{stderr}"),
        other => panic!("unexpected {other:?}"),
    }
}

/// The upload-pack wrapper sleeps before serving and records its pid, so
/// the test can check that cancelling also stopped Git's child processes.
#[cfg(unix)]
#[test]
fn cancelling_a_slow_fetch_stops_the_whole_process_tree() {
    use std::time::{Duration, Instant};

    let (remote, local) = remote_and_local();
    let pid_file = local.path().join("upload-pack.pid");
    let upload_pack = format!(
        "sh -c 'echo $$ > {}; sleep 30; exec git-upload-pack \"$@\"' upload-pack",
        pid_file.display()
    );
    let upload_arg = format!("--upload-pack={upload_pack}");
    let url = remote.path().to_str().unwrap().to_owned();

    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    let watch = pid_file.clone();
    std::thread::spawn(move || {
        // Cancel once the slow helper is running.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !watch.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(100));
        trigger.cancel();
    });

    let start = Instant::now();
    let result = CliBackend::new(local.path()).run_network(
        &["fetch", "--progress", &upload_arg, &url],
        &cancel,
        &mut |_| {},
    );
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "cancel took too long"
    );

    let pid = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .to_owned();
    std::thread::sleep(Duration::from_millis(200));
    let alive = std::process::Command::new("kill")
        .args(["-0", &pid])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(!alive, "upload-pack helper {pid} survived the cancel");
}

#[test]
fn cancel_before_start_stops_immediately() {
    let (_remote, local) = remote_and_local();
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = CliBackend::new(local.path()).run_network(
        &["fetch", "--progress", "origin"],
        &cancel,
        &mut |_| {},
    );
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
}

/// Local clone of a bare remote, with `main` tracking `origin/main`.
fn clone_of_bare() -> (TestRepo, TestRepo, TestRepo) {
    let upstream = TestRepo::new();
    upstream.commit("a.txt", "a\n", "Base");
    let bare = TestRepo::new_bare();
    upstream.git(&["push", "--quiet", bare.path().to_str().unwrap(), "main"]);
    let local = TestRepo::new();
    local.git(&["remote", "add", "origin", bare.path().to_str().unwrap()]);
    local.git(&["fetch", "--quiet", "origin"]);
    local.git(&["switch", "--quiet", "--track", "-C", "main", "origin/main"]);
    (upstream, bare, local)
}

fn fetch_all(local: &TestRepo) {
    CliBackend::new(local.path())
        .run_network(
            &["fetch", "--all", "--progress"],
            &CancelToken::new(),
            &mut |_| {},
        )
        .unwrap();
}

#[test]
fn fetch_all_updates_remote_branches_and_ahead_behind() {
    use git_core::GitBackend as _;
    let (upstream, bare, local) = clone_of_bare();
    upstream.commit("b.txt", "b\n", "Remote change");
    upstream.git(&["push", "--quiet", bare.path().to_str().unwrap(), "main"]);
    upstream.git(&[
        "push",
        "--quiet",
        bare.path().to_str().unwrap(),
        "main:feature",
    ]);

    fetch_all(&local);
    let refs = git_core::GixBackend::discover(local.path())
        .unwrap()
        .references()
        .unwrap();
    let main = refs.iter().find(|r| r.name == "main").unwrap();
    assert_eq!(main.ahead_behind, Some((0, 1)));
    assert!(refs.iter().any(|r| r.name == "origin/feature"));
}

#[test]
fn fetch_prunes_only_when_configured() {
    let (_upstream, bare, local) = clone_of_bare();
    bare.git(&["branch", "gone", "main"]);
    fetch_all(&local);
    bare.git(&["branch", "-D", "gone"]);

    fetch_all(&local);
    assert!(local.try_git(&["rev-parse", "--verify", "--quiet", "origin/gone"]));
    local.git(&["config", "fetch.prune", "true"]);
    fetch_all(&local);
    assert!(!local.try_git(&["rev-parse", "--verify", "--quiet", "origin/gone"]));
}

/// Like [`clone_of_bare`], with an identity so pulls can create commits.
fn pull_setup() -> (TestRepo, TestRepo, TestRepo) {
    let (upstream, bare, local) = clone_of_bare();
    local.git(&["config", "user.name", "Gitloom Test"]);
    local.git(&["config", "user.email", "test@example.com"]);
    local.git(&["config", "commit.gpgsign", "false"]);
    (upstream, bare, local)
}

fn push_upstream(upstream: &TestRepo, bare: &TestRepo, file: &str, content: &str) {
    upstream.commit(file, content, "Upstream change");
    upstream.git(&["push", "--quiet", bare.path().to_str().unwrap(), "main"]);
}

fn pull(local: &TestRepo, mode: git_core::PullMode) -> git_core::Result<()> {
    CliBackend::new(local.path())
        .run_network(&mode.args(), &CancelToken::new(), &mut |_| {})
        .map(drop)
}

fn state(repo: &TestRepo) -> git_core::RepoState {
    use git_core::GitBackend as _;
    git_core::GixBackend::discover(repo.path())
        .unwrap()
        .state()
        .unwrap()
}

#[test]
fn pull_fast_forwards_in_every_mode() {
    for mode in git_core::PullMode::ALL {
        let (upstream, bare, local) = pull_setup();
        push_upstream(&upstream, &bare, "b.txt", "b\n");
        pull(&local, mode).unwrap();
        assert_eq!(local.head(), upstream.head(), "{mode:?}");
    }
}

#[test]
fn diverged_branches_per_mode() {
    use git_core::PullMode;
    // ff if possible: merge commit with both parents.
    let (upstream, bare, local) = pull_setup();
    push_upstream(&upstream, &bare, "b.txt", "b\n");
    local.commit("c.txt", "c\n", "Local change");
    pull(&local, PullMode::FastForwardIfPossible).unwrap();
    assert_eq!(local.parents("HEAD").len(), 2);

    // ff only: fails and changes nothing.
    let (upstream, bare, local) = pull_setup();
    push_upstream(&upstream, &bare, "b.txt", "b\n");
    let before = local.commit("c.txt", "c\n", "Local change");
    match pull(&local, PullMode::FastForwardOnly) {
        Err(Error::Command { stderr, .. }) => {
            assert!(stderr.contains("Not possible to fast-forward"), "{stderr}")
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(local.head(), before);

    // rebase: local commit replayed on top of the upstream commit.
    let (upstream, bare, local) = pull_setup();
    push_upstream(&upstream, &bare, "b.txt", "b\n");
    local.commit("c.txt", "c\n", "Local change");
    pull(&local, PullMode::Rebase).unwrap();
    assert_eq!(local.parents("HEAD"), [upstream.head()]);
}

#[test]
fn conflicting_pull_can_be_aborted() {
    use git_core::{PullMode, RepoState};
    for mode in [PullMode::FastForwardIfPossible, PullMode::Rebase] {
        let (upstream, bare, local) = pull_setup();
        push_upstream(&upstream, &bare, "a.txt", "upstream\n");
        let before = local.commit("a.txt", "local\n", "Local change");
        assert!(pull(&local, mode).is_err(), "{mode:?}");

        let cli = CliBackend::new(local.path());
        match state(&local) {
            RepoState::Merging { .. } => cli.abort_merge().unwrap(),
            RepoState::Rebasing { .. } => cli.abort_rebase().unwrap(),
            other => panic!("{mode:?}: unexpected state {other:?}"),
        }
        assert_eq!(state(&local), RepoState::Clean);
        assert_eq!(local.head(), before, "{mode:?}");
    }
}

fn push(local: &TestRepo, branch: &str, upstream: bool, lease: bool) -> git_core::Result<()> {
    let args = git_core::push_args("origin", branch, branch, upstream, lease);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    CliBackend::new(local.path())
        .run_network(&args, &CancelToken::new(), &mut |_| {})
        .map(drop)
}

fn remote_head(bare: &TestRepo, branch: &str) -> String {
    bare.git(&["rev-parse", branch])
}

#[test]
fn push_updates_the_upstream() {
    let (_upstream, bare, local) = pull_setup();
    let head = local.commit("c.txt", "c\n", "Local change");
    push(&local, "main", false, false).unwrap();
    assert_eq!(remote_head(&bare, "main"), head);
}

#[test]
fn push_new_branch_sets_its_upstream() {
    let (_upstream, bare, local) = pull_setup();
    local.git(&["switch", "--quiet", "-c", "topic"]);
    let head = local.commit("t.txt", "t\n", "Topic");
    push(&local, "topic", true, false).unwrap();
    assert_eq!(remote_head(&bare, "topic"), head);
    assert_eq!(
        local.git(&["rev-parse", "--abbrev-ref", "topic@{upstream}"]),
        "origin/topic"
    );
}

#[test]
fn rejected_push_then_force_with_lease() {
    let (upstream, bare, local) = pull_setup();
    push_upstream(&upstream, &bare, "b.txt", "b\n");
    local.git(&["fetch", "--quiet", "origin"]);
    let head = local.commit("c.txt", "c\n", "Local change");

    match push(&local, "main", false, false) {
        Err(Error::Command { stderr, .. }) => assert_eq!(
            git_core::push_rejection(&stderr),
            Some(git_core::PushRejection::NonFastForward),
            "{stderr}"
        ),
        other => panic!("unexpected {other:?}"),
    }
    // The remote is where our remote-tracking branch says: the lease holds.
    push(&local, "main", false, true).unwrap();
    assert_eq!(remote_head(&bare, "main"), head);
}

#[test]
fn force_with_lease_refuses_when_the_remote_moved() {
    let (upstream, bare, local) = pull_setup();
    // Someone pushes after our last fetch.
    push_upstream(&upstream, &bare, "b.txt", "b\n");
    let theirs = remote_head(&bare, "main");
    local.commit("c.txt", "c\n", "Local change");

    match push(&local, "main", false, true) {
        Err(Error::Command { stderr, .. }) => assert_eq!(
            git_core::push_rejection(&stderr),
            Some(git_core::PushRejection::StaleLease),
            "{stderr}"
        ),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(remote_head(&bare, "main"), theirs, "their commit survived");
}

fn network(local: &TestRepo, args: Vec<String>) -> git_core::Result<()> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    CliBackend::new(local.path())
        .run_network(&args, &CancelToken::new(), &mut |_| {})
        .map(drop)
}

#[test]
fn rename_branch_locally_and_on_the_remote() {
    let (_upstream, bare, local) = pull_setup();
    local.git(&["switch", "--quiet", "-c", "old-name"]);
    let head = local.commit("o.txt", "o\n", "On old");
    push(&local, "old-name", true, false).unwrap();

    let cli = CliBackend::new(local.path());
    cli.rename_branch("old-name", "new-name").unwrap();
    // Locally renamed; the upstream still points at the old remote branch.
    assert_eq!(
        local.git(&["rev-parse", "--abbrev-ref", "new-name@{upstream}"]),
        "origin/old-name"
    );
    network(
        &local,
        git_core::rename_remote_branch_args("origin", "new-name", "old-name"),
    )
    .unwrap();
    assert_eq!(remote_head(&bare, "new-name"), head);
    assert!(!bare.try_git(&["rev-parse", "--verify", "--quiet", "old-name"]));
    assert_eq!(
        local.git(&["rev-parse", "--abbrev-ref", "new-name@{upstream}"]),
        "origin/new-name"
    );
}

#[test]
fn delete_branches_locally_and_on_the_remote() {
    let (_upstream, bare, local) = pull_setup();
    let cli = CliBackend::new(local.path());
    // Merged branch: plain delete works.
    local.git(&["branch", "merged"]);
    cli.delete_branch("merged", false).unwrap();

    // Unmerged branch: refused without force.
    local.git(&["switch", "--quiet", "-c", "unmerged"]);
    local.commit("u.txt", "u\n", "Unmerged");
    local.git(&["switch", "--quiet", "main"]);
    match cli.delete_branch("unmerged", false) {
        Err(Error::Command { stderr, .. }) => {
            assert!(git_core::branch_not_merged(&stderr), "{stderr}")
        }
        other => panic!("unexpected {other:?}"),
    }
    cli.delete_branch("unmerged", true).unwrap();
    assert!(!local.try_git(&["rev-parse", "--verify", "--quiet", "unmerged"]));

    // Remote branch.
    bare.git(&["branch", "gone", "main"]);
    network(
        &local,
        git_core::delete_remote_branch_args("origin", "gone"),
    )
    .unwrap();
    assert!(!bare.try_git(&["rev-parse", "--verify", "--quiet", "gone"]));
}
