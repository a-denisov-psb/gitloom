//! Cherry-pick and revert (M4-09). Blocking: runs `git`.

use std::path::Path;

use crate::merge::MergeOutcome;
use crate::{GitWriter, Operation, Result, RunOptions};

fn read_only() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

/// Applies the commits `ids` in the given order (oldest first) on HEAD.
/// A stop for conflicts or an empty result leaves the cherry-pick in
/// progress and returns [`MergeOutcome::Conflicts`].
pub fn cherry_pick(git: &dyn GitWriter, ids: &[String]) -> Result<MergeOutcome> {
    if ids.is_empty() {
        return Ok(MergeOutcome::Done);
    }
    let mut args = vec!["cherry-pick"];
    args.extend(ids.iter().map(String::as_str));
    finish(
        git,
        git.run(&args, RunOptions::default()),
        Operation::CherryPick,
    )
}

/// Reverts commit `id` with Git's default message; `mainline` (1-based) is
/// the parent a merge commit is reverted against.
pub fn revert(git: &dyn GitWriter, id: &str, mainline: Option<usize>) -> Result<MergeOutcome> {
    let number = mainline.map(|n| n.to_string());
    let mut args = vec!["revert", "--no-edit"];
    if let Some(number) = &number {
        args.extend(["-m", number.as_str()]);
    }
    args.push(id);
    finish(
        git,
        git.run(&args, RunOptions::default()),
        Operation::Revert,
    )
}

/// "Commit empty": records the current (empty) cherry-pick or revert step
/// as an empty commit with the prepared message, then continues with the
/// remaining commits, if any.
pub fn commit_empty(git: &dyn GitWriter, operation: Operation) -> Result<MergeOutcome> {
    git.run(
        &["commit", "--allow-empty", "--no-edit"],
        RunOptions::default(),
    )?;
    if !sequencer_running(git)? {
        return Ok(MergeOutcome::Done);
    }
    let result = git.run(&[operation.command(), "--continue"], RunOptions::default());
    finish(git, result, operation)
}

fn finish(
    git: &dyn GitWriter,
    result: Result<crate::GitOutput>,
    operation: Operation,
) -> Result<MergeOutcome> {
    match result {
        Ok(_) => Ok(MergeOutcome::Done),
        Err(error) => {
            let head = match operation {
                Operation::Revert => "REVERT_HEAD",
                _ => "CHERRY_PICK_HEAD",
            };
            let stopped = git
                .run(&["rev-parse", "--verify", "--quiet", head], read_only())
                .is_ok();
            if stopped {
                Ok(MergeOutcome::Conflicts)
            } else {
                Err(error)
            }
        }
    }
}

/// A multi-commit cherry-pick or revert has steps left.
fn sequencer_running(git: &dyn GitWriter) -> Result<bool> {
    let output = git.run(&["rev-parse", "--absolute-git-dir"], read_only())?;
    let dir = output.stdout_text();
    Ok(Path::new(dir.trim_end())
        .join("sequencer")
        .join("todo")
        .is_file())
}
