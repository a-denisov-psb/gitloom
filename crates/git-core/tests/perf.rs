//! Manual timing checks against a large real repository.
//!
//! ```sh
//! GITLOOM_PERF_REPO=/path/to/rust cargo test -p git-core --release --test perf -- --ignored --nocapture
//! ```

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use git_core::{
    build_patch, CliBackend, DiffKind, DiffRequest, GitBackend, GitWriter, GixBackend,
    PatchDirection,
};
use support::TestRepo;

/// Median of `runs` timings of `f`.
fn median(runs: usize, mut f: impl FnMut() -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..runs).map(|_| f()).collect();
    times.sort();
    times[runs / 2]
}

fn repo() -> Option<GixBackend> {
    let path = std::env::var("GITLOOM_PERF_REPO").ok()?;
    Some(GixBackend::discover(path).unwrap())
}

#[test]
#[ignore = "manual timing, needs GITLOOM_PERF_REPO"]
fn open_and_log_100k() {
    let Some(_) = repo() else { return };
    let start = Instant::now();
    let backend = repo().unwrap();
    let log = backend.log(100_000).unwrap();
    let refs = backend.references().unwrap();
    eprintln!(
        "open + log({}) + refs({}) in {:?}, truncated: {}",
        log.commits.len(),
        refs.len(),
        start.elapsed(),
        log.truncated
    );
}

#[test]
#[ignore = "manual timing, needs GITLOOM_PERF_REPO"]
fn status() {
    let Some(backend) = repo() else { return };
    let start = Instant::now();
    let tree = backend.working_tree().unwrap();
    eprintln!(
        "status: {} unstaged, {} staged in {:?}",
        tree.unstaged.len(),
        tree.staged.len(),
        start.elapsed()
    );
}

#[test]
#[ignore = "manual check, needs GITLOOM_PERF_REPO"]
fn ignored_build_output_causes_no_refresh() {
    let Some(backend) = repo() else { return };
    let root = backend.path().to_path_buf();
    let paths: Vec<PathBuf> = (0..10_000)
        .map(|i| root.join(format!("build/x86_64-unknown-linux-gnu/stage1/obj{i}.o")))
        .chain((0..1_000).map(|i| root.join(format!("target/debug/deps/lib{i}.rlib"))))
        .collect();
    let start = Instant::now();
    let changes = backend.classify_changes(&paths);
    eprintln!(
        "classified {} ignored paths in {:?}: {changes:?}",
        paths.len(),
        start.elapsed()
    );
    assert!(changes.is_empty());
}

/// Large file (just under the 2 MB diff limit) with changes spread over it;
/// stages one hunk the way the diff view does.
#[test]
#[ignore = "manual timing"]
fn stage_hunk_of_a_large_file() {
    let repo = TestRepo::new();
    let lines = 34_000;
    let line = |i: usize| format!("line {i:06} with some text to reach a realistic length\n");
    let original: String = (0..lines).map(line).collect();
    repo.commit("big.txt", &original, "Base");
    let changed: String = (0..lines)
        .map(|i| {
            if i % 100 == 0 {
                format!("changed {i}\n")
            } else {
                line(i)
            }
        })
        .collect();
    repo.write("big.txt", &changed);
    eprintln!("file size {} KB", changed.len() / 1024);

    let backend = GixBackend::discover(repo.path()).unwrap();
    let cli = CliBackend::new(repo.path());
    let request = DiffRequest {
        kind: DiffKind::Unstaged,
        path: "big.txt".into(),
        old_path: None,
    };
    let mut hunks = 0;
    let diff_time = median(5, || {
        let start = Instant::now();
        hunks = backend.file_diff(&request).unwrap().hunks.len();
        start.elapsed()
    });
    let stage_time = median(5, || {
        let diff = backend.file_diff(&request).unwrap();
        let start = Instant::now();
        let patch = build_patch(&diff, PatchDirection::Stage, |h, _| h == hunks / 2).unwrap();
        cli.apply_cached(&patch, false).unwrap();
        let elapsed = start.elapsed();
        cli.unstage(&["big.txt".to_owned()]).unwrap();
        elapsed
    });
    eprintln!("{hunks} hunks: diff {diff_time:?}, build patch + git apply --cached {stage_time:?}");
}

/// Overhead of the network runner (stderr streaming, progress parsing,
/// cancel polling) against plain `git` for the same fetch from a local remote.
#[test]
#[ignore = "manual timing"]
fn network_runner_overhead() {
    use git_core::CancelToken;
    use std::process::Command;

    let remote = TestRepo::new();
    for i in 0..300 {
        remote.commit(
            &format!("f{i}.txt"),
            &format!("{i}\n").repeat(2000),
            "Commit",
        );
    }
    let url = remote.path().to_str().unwrap().to_owned();
    let fresh = || {
        let local = TestRepo::new();
        local.git(&["remote", "add", "origin", &url]);
        local
    };
    let plain = median(5, || {
        let local = fresh();
        let start = Instant::now();
        let status = Command::new("git")
            .args(["fetch", "--quiet", "origin"])
            .current_dir(local.path())
            .status()
            .unwrap();
        assert!(status.success());
        start.elapsed()
    });
    let mut updates = 0;
    let runner = median(5, || {
        let local = fresh();
        let cli = CliBackend::new(local.path());
        let start = Instant::now();
        cli.run_network(
            &["fetch", "--progress", "origin"],
            &CancelToken::new(),
            &mut |_| updates += 1,
        )
        .unwrap();
        start.elapsed()
    });
    eprintln!("fetch of 300 commits: plain git {plain:?}, runner {runner:?} ({updates} progress updates over 5 runs)");
}

/// M4-14: conflict view data for a 5 000-line file with 500 conflicts.
#[test]
#[ignore = "manual timing"]
fn conflict_of_a_5000_line_file() {
    use git_core::{load_conflict, ConflictContent, Picks, Side};
    let lines = |tag: &str| {
        (0..5000)
            .map(|i| {
                if i % 10 == 5 {
                    format!("line {i} {tag}\n")
                } else {
                    format!("line {i}\n")
                }
            })
            .collect::<String>()
    };
    let repo = TestRepo::new();
    repo.commit("big.txt", &lines("base"), "base");
    repo.branch("topic");
    repo.commit("big.txt", &lines("ours"), "ours");
    repo.checkout("topic");
    repo.commit("big.txt", &lines("theirs"), "theirs");
    repo.checkout("main");
    assert!(!repo.try_git(&["merge", "topic"]));
    let cli = CliBackend::new(repo.path());

    let load = median(5, || {
        let start = Instant::now();
        load_conflict(&cli, "big.txt").unwrap();
        start.elapsed()
    });
    let ConflictContent::Text(text) = load_conflict(&cli, "big.txt").unwrap().content else {
        panic!("expected a text conflict");
    };
    let output = median(5, || {
        let mut picks = Picks::new(&text);
        let start = Instant::now();
        picks.take_all(Side::Theirs);
        let out = text.output(&picks, ("HEAD", "topic"));
        assert!(!out.is_empty());
        start.elapsed()
    });
    eprintln!(
        "5000 lines, {} conflicts: load {load:?}, picks + output {output:?}",
        text.hunks().count()
    );
}
