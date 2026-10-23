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
