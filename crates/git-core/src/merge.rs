//! Merge and fast-forward (M4-04). Blocking: runs `git`.

use crate::{Error, GitWriter, Result, RunOptions};

/// How a merge ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Fast-forwarded or merge commit created.
    Merged,
    /// Stopped with conflicts; the merge is in progress (`MERGE_HEAD`).
    Conflicts,
}

/// Merges `rev` into HEAD (`git merge`), respecting the user's `merge.ff`
/// and `merge.autoStash`; the message is Git's default. `autostash` stashes
/// local changes first and re-applies them afterwards (if they conflict with
/// the result, Git keeps them in the stash list).
pub fn merge(git: &dyn GitWriter, rev: &str, autostash: bool) -> Result<MergeOutcome> {
    let mut args = vec!["merge"];
    if autostash {
        args.push("--autostash");
    }
    args.push(short_branch(rev));
    match git.run(&args, RunOptions::default()) {
        Ok(_) => Ok(MergeOutcome::Merged),
        Err(error) => {
            if merge_in_progress(git)? {
                Ok(MergeOutcome::Conflicts)
            } else {
                Err(error)
            }
        }
    }
}

/// Fast-forwards the checked-out branch to `rev` (`git merge --ff-only`).
pub fn fast_forward_head(git: &dyn GitWriter, rev: &str, autostash: bool) -> Result<()> {
    let mut args = vec!["merge", "--ff-only"];
    if autostash {
        args.push("--autostash");
    }
    args.push(short_branch(rev));
    git.run(&args, RunOptions::default()).map(drop)
}

/// Branch name as Git's default merge message shows it: `refs/heads/x` →
/// `x` ("Merge branch 'x'"), `refs/remotes/origin/x` → `origin/x`
/// ("Merge remote-tracking branch 'origin/x'").
fn short_branch(rev: &str) -> &str {
    rev.strip_prefix("refs/heads/")
        .or_else(|| rev.strip_prefix("refs/remotes/"))
        .unwrap_or(rev)
}

/// Fast-forwards local branch `branch` (not checked out) to the reference
/// `source` (full name, e.g. `refs/remotes/origin/main`) without touching
/// the working tree. Git refuses if it is not a fast-forward.
pub fn fast_forward_branch(git: &dyn GitWriter, branch: &str, source: &str) -> Result<()> {
    let refspec = format!("{source}:refs/heads/{branch}");
    git.run(
        &["fetch", "--quiet", "--no-write-fetch-head", ".", &refspec],
        RunOptions::default(),
    )
    .map(drop)
}

/// True if a merge stopped and waits for its conclusion.
fn merge_in_progress(git: &dyn GitWriter) -> Result<bool> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    match git.run(&["rev-parse", "--verify", "--quiet", "MERGE_HEAD"], read) {
        Ok(_) => Ok(true),
        Err(Error::Command { code: Some(1), .. }) => Ok(false),
        Err(error) => Err(error),
    }
}

/// True if Git refused to merge (or fast-forward) because local changes
/// would be overwritten.
pub fn merge_blocked(stderr: &str) -> bool {
    stderr.contains("would be overwritten by merge")
        || stderr.contains("untracked working tree files would be overwritten")
        || stderr.contains("Please commit your changes or stash them")
}
