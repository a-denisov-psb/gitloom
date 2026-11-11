//! Clone through the network path (M10-02, ADR-045).

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::clone::{clone, repo_name};
use git_core::{CancelToken, CliBackend, Error, Progress};
use support::TestRepo;

fn source() -> TestRepo {
    let repo = TestRepo::new();
    for i in 0..10 {
        repo.commit(&format!("f{i}.txt"), &"content\n".repeat(100 + i), "Commit");
    }
    repo
}

/// `file:///` URL of a local folder (also `file:///C:/…` on Windows).
fn file_url(path: &std::path::Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", path.trim_start_matches('/'))
}

#[test]
fn clone_from_a_local_repository_reports_progress() {
    let source = source();
    let parent = tempfile::tempdir().unwrap();
    let url = source.path().to_str().unwrap().to_owned();
    let target = parent.path().join(repo_name(&url).unwrap());
    let mut updates: Vec<Progress> = Vec::new();
    clone(
        &CliBackend::new(parent.path()),
        &file_url(source.path()),
        &target,
        false,
        &CancelToken::new(),
        &mut |p| updates.push(p),
    )
    .unwrap();
    assert!(target.join(".git").is_dir());
    assert!(target.join("f9.txt").is_file());
    assert!(!updates.is_empty(), "progress reported");
}

#[test]
fn clone_with_submodules() {
    let sub = source();
    let main = TestRepo::new();
    main.commit("readme.txt", "main\n", "Main");
    main.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        sub.path().to_str().unwrap(),
        "lib",
    ]);
    main.git(&["commit", "--quiet", "-m", "Add submodule"]);
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("main");
    // Local submodule URLs need the file protocol allowed (Git ≥ 2.38.1).
    let git = CliBackend::new(parent.path()).with_config("protocol.file.allow", "always");
    let result = clone(
        &git,
        main.path().to_str().unwrap(),
        &target,
        true,
        &CancelToken::new(),
        &mut |_| {},
    );
    result.unwrap();
    assert!(target.join("lib/f0.txt").is_file(), "submodule checked out");
}

#[test]
fn non_empty_target_is_refused_and_kept() {
    let source = source();
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("busy");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("mine.txt"), "keep\n").unwrap();
    let error = clone(
        &CliBackend::new(parent.path()),
        source.path().to_str().unwrap(),
        &target,
        false,
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("not empty"), "{error}");
    assert!(target.join("mine.txt").is_file());
}

#[test]
fn failed_clone_leaves_no_new_folder() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("missing");
    let error = clone(
        &CliBackend::new(parent.path()),
        parent.path().join("no-such-repo").to_str().unwrap(),
        &target,
        false,
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(error, Error::Command { .. }), "{error:?}");
    assert!(!target.exists());
}

/// A cancelled clone removes the folder it created: the remote is an
/// `ext::` command that never answers, so the clone is still running when
/// it is cancelled.
#[cfg(unix)]
#[test]
fn cancelled_clone_removes_the_new_folder() {
    use std::os::unix::fs::PermissionsExt as _;
    use std::time::{Duration, Instant};
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("cancelled");
    let marker = parent.path().join("started");
    let script = parent.path().join("slow-remote.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\nsleep 30\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    let watch = marker.clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !watch.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        trigger.cancel();
    });
    let git = CliBackend::new(parent.path()).with_config("protocol.ext.allow", "always");
    let url = format!("ext::{}", script.display());
    let started = Instant::now();
    let result = clone(&git, &url, &target, false, &cancel, &mut |_| {});
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
    assert!(
        marker.exists(),
        "the clone was running when it was cancelled"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(!target.exists(), "no folder left behind");
}
