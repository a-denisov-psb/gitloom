//! Manual timing checks of M4 operations with the real binary as editor.
//!
//! ```sh
//! cargo test -p gitloom --release --test perf -- --ignored --nocapture
//! ```

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../../git-core/tests/support/mod.rs"]
mod support;

use std::time::Instant;

use git_core::{build_todo, default_message, rebase_commits, rebase_interactive, CliBackend};
use git_core::{TodoAction, TodoEntry};
use support::TestRepo;

/// M4-14: interactive rebase of 200 commits (every 10th squashed, every
/// 20th reworded, the first two swapped).
#[test]
#[ignore = "manual timing"]
fn interactive_rebase_of_200_commits() {
    let repo = TestRepo::new();
    repo.git(&["config", "user.name", "Test Author"]);
    repo.git(&["config", "user.email", "author@example.com"]);
    let base = repo.commit("base.txt", "base\n", "base");
    for i in 0..200 {
        repo.commit(
            &format!("f{i}.txt"),
            &format!("{i}\n"),
            &format!("commit {i}"),
        );
    }
    let cli = CliBackend::new(repo.path()).with_editor(env!("CARGO_BIN_EXE_gitloom"));

    let start = Instant::now();
    let commits = rebase_commits(&cli, &base).unwrap();
    let load = start.elapsed();
    let mut entries: Vec<TodoEntry> = commits
        .into_iter()
        .enumerate()
        .map(|(i, (id, message))| TodoEntry {
            id,
            action: if i > 0 && i % 10 == 0 {
                TodoAction::Squash
            } else if i % 20 == 5 {
                TodoAction::Reword
            } else {
                TodoAction::Pick
            },
            message,
        })
        .collect();
    entries.swap(1, 2);
    let input = build_todo(&entries, |head| default_message(&entries, head)).unwrap();
    let start = Instant::now();
    rebase_interactive(&cli, &base, &input, false).unwrap();
    let run = start.elapsed();
    let count = repo.git(&["rev-list", "--count", &format!("{base}..HEAD")]);
    eprintln!(
        "200 commits: load {load:?}, rebase {run:?} ({} messages, {count} commits after)",
        input.messages.len()
    );
}
