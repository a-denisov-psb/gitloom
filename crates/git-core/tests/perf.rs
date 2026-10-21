//! Manual timing checks against a large real repository.
//!
//! ```sh
//! GITLOOM_PERF_REPO=/path/to/rust cargo test -p git-core --release --test perf -- --ignored --nocapture
//! ```

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Instant;

use git_core::{GitBackend, GixBackend};

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
