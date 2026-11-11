//! Undo and redo of history operations (M4-12): merge, fast-forward,
//! rebase, interactive rebase, cherry-pick, revert, reset, reword and
//! squash. Blocking: runs `git` and writes files.

use std::path::Path;

use crate::reset::{hard_reset_with_snapshot, reset, undo_hard_reset, ResetMode, ResetSnapshot};
use crate::{head_commit, Error, GitWriter, Oid, RepoState, Result, RunOptions};

/// How an operation is undone and redone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryUndo {
    /// HEAD moved and the working tree followed (merge, rebase, cherry-pick,
    /// …): `git reset --keep`, which refuses to overwrite local changes.
    Keep,
    /// Soft or mixed reset; `index_tree` is the index before it, without
    /// the `unmerged` entries recorded separately (M9-04).
    Reset {
        mode: ResetMode,
        index_tree: Oid,
        unmerged: Vec<crate::IndexSnapshot>,
    },
    /// Hard reset with everything it overwrote.
    HardReset(ResetSnapshot),
    /// A branch that is not checked out was fast-forwarded.
    MoveBranch { name: String },
}

/// One finished history operation on the undo or redo stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryChange {
    /// Shown in the Undo/Redo tooltips, e.g. "Merge topic into main".
    pub label: String,
    pub kind: HistoryUndo,
    pub before: Oid,
    pub after: Oid,
    /// Checked-out branch the operation ran on (`None`: detached HEAD).
    pub branch: Option<String>,
}

const CHANGED: &str = "Cannot undo: the repository changed";

/// Checked-out branch (short name), `None` for a detached HEAD.
pub fn current_branch(git: &dyn GitWriter) -> Result<Option<String>> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    match git.run(&["symbolic-ref", "--quiet", "--short", "HEAD"], read) {
        Ok(output) => Ok(Some(output.stdout_text().trim().to_owned())),
        Err(Error::Command { code: Some(1), .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Operation in progress, read like [`crate::GitBackend::state`].
pub fn current_state(git: &dyn GitWriter) -> Result<RepoState> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let dir = git.run(&["rev-parse", "--absolute-git-dir"], read)?;
    Ok(crate::state::detect(Path::new(
        dir.stdout_text().trim_end(),
    )))
}

fn branch_commit(git: &dyn GitWriter, name: &str) -> Result<Option<Oid>> {
    crate::ref_commit(git, &format!("refs/heads/{name}"))
}

/// HEAD (or the moved branch) is where `expected` says, on the same branch.
fn check(git: &dyn GitWriter, change: &HistoryChange, expected: &str) -> Result<()> {
    let at = match &change.kind {
        HistoryUndo::MoveBranch { name } => branch_commit(git, name)?,
        _ => {
            if current_branch(git)? != change.branch {
                return Err(Error::Backend(CHANGED.into()));
            }
            head_commit(git)?
        }
    };
    if at.as_deref() != Some(expected) {
        return Err(Error::Backend(CHANGED.into()));
    }
    if !current_state(git)?.is_clean() {
        return Err(Error::Backend(
            "Cannot undo while an operation is in progress".into(),
        ));
    }
    Ok(())
}

fn move_branch(git: &dyn GitWriter, name: &str, from: &str, to: &str) -> Result<()> {
    let reference = format!("refs/heads/{name}");
    git.run(
        &[
            "update-ref",
            "-m",
            &crate::identity::reflog_message("undo"),
            &reference,
            to,
            from,
        ],
        RunOptions::default(),
    )
    .map(drop)
}

/// Undoes `change`; returns the entry for the redo stack.
pub fn undo_history(
    git: &dyn GitWriter,
    root: &Path,
    change: &HistoryChange,
) -> Result<HistoryChange> {
    check(git, change, &change.after)?;
    match &change.kind {
        HistoryUndo::Keep => reset_keep(git, &change.before)?,
        HistoryUndo::Reset {
            index_tree,
            unmerged,
            ..
        } => {
            reset(git, &change.before, ResetMode::Soft)?;
            crate::restore_index(git, index_tree, unmerged)?;
        }
        HistoryUndo::HardReset(snapshot) => undo_hard_reset(git, root, snapshot)?,
        HistoryUndo::MoveBranch { name } => move_branch(git, name, &change.after, &change.before)?,
    }
    Ok(change.clone())
}

/// Redoes an undone `change`; returns the entry for the undo stack (a hard
/// reset records a fresh snapshot).
pub fn redo_history(
    git: &dyn GitWriter,
    root: &Path,
    change: &HistoryChange,
) -> Result<HistoryChange> {
    check(git, change, &change.before)?;
    let mut redone = change.clone();
    match &change.kind {
        HistoryUndo::Keep => reset_keep(git, &change.after)?,
        HistoryUndo::Reset { mode, .. } => reset(git, &change.after, *mode)?,
        HistoryUndo::HardReset(_) => {
            redone.kind =
                HistoryUndo::HardReset(hard_reset_with_snapshot(git, root, &change.after)?);
        }
        HistoryUndo::MoveBranch { name } => move_branch(git, name, &change.before, &change.after)?,
    }
    Ok(redone)
}

fn reset_keep(git: &dyn GitWriter, target: &str) -> Result<()> {
    git.run(
        &["reset", "--quiet", "--keep", target, "--"],
        RunOptions::default(),
    )
    .map(drop)
}

/// The index as a tree, recorded before a soft or mixed reset.
pub fn index_tree(git: &dyn GitWriter) -> Result<Oid> {
    Ok(git
        .run(&["write-tree"], RunOptions::default())?
        .stdout_text()
        .trim()
        .to_owned())
}
