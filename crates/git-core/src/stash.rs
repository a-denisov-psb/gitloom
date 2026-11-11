//! Stash pop and apply that may stop in conflicts (M9-01), with a snapshot
//! that makes them undoable and redoable (M9-02, ADR-043).
//!
//! Blocking: runs `git` and reads/writes files.

use std::path::Path;

use crate::reset::{capture_file, restore_file, RawFile};
use crate::{head_commit, Error, GitWriter, Oid, Result, RunOptions};

/// How a stash pop or apply ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashOutcome {
    /// Applied cleanly (a pop also dropped the stash).
    Applied,
    /// Stopped with conflicted files; Git kept the stash.
    Conflicts,
}

/// Everything a stash pop or apply changed, recorded so it can be undone:
/// HEAD, the index (as a tree), the files the stash touches and the stash
/// entry itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashRecord {
    /// `git stash pop` (true) or `git stash apply`.
    pub pop: bool,
    /// Commit of the stash entry.
    pub stash: Oid,
    /// Reflog message of the entry ("WIP on main: …"), for `git stash store`.
    pub message: String,
    /// Position of the entry in the stash list before the command.
    pub position: usize,
    /// HEAD before (and, as a precondition of undo, after) the command.
    pub head: Oid,
    /// The index before the command, written as a tree.
    pub index_tree: Oid,
    /// Files the stash touches, as they were before the command.
    pub before: Vec<RawFile>,
    /// The same files right after the command.
    pub after: Vec<RawFile>,
    pub outcome: StashOutcome,
    /// The command removed the entry from the stash list.
    pub dropped: bool,
}

impl StashRecord {
    /// "Pop stash" or "Apply stash", for undo labels.
    pub fn label(&self) -> &'static str {
        if self.pop {
            "Pop stash"
        } else {
            "Apply stash"
        }
    }
}

fn read_only() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

fn nul_list(stdout: &[u8]) -> Vec<String> {
    stdout
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// Paths with unmerged index entries (stages 1–3), sorted and unique.
pub fn unmerged_paths(git: &dyn GitWriter) -> Result<Vec<String>> {
    let output = git.run(&["ls-files", "--unmerged", "-z"], read_only())?;
    let mut paths: Vec<String> = nul_list(&output.stdout)
        .into_iter()
        .filter_map(|record| record.split_once('\t').map(|(_, path)| path.to_owned()))
        .collect();
    paths.dedup();
    Ok(paths)
}

/// The stash list as (commit, reflog message), newest first.
fn stash_entries(git: &dyn GitWriter) -> Result<Vec<(Oid, String)>> {
    let output = git.run(&["stash", "list", "--format=%H%x00%gs%x00"], read_only())?;
    let fields = output
        .stdout
        .split(|&b| b == 0)
        .map(|f| String::from_utf8_lossy(f).trim().to_owned())
        .collect::<Vec<_>>();
    Ok(fields
        .chunks(2)
        .filter(|pair| pair.len() == 2 && !pair[0].is_empty())
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect())
}

/// Paths a pop or apply of `stash` can change: its working tree and index
/// changes against its base, and its untracked files.
fn stash_paths(git: &dyn GitWriter, stash: &str) -> Result<Vec<String>> {
    let base = format!("{stash}^1");
    let mut paths = Vec::new();
    for side in [stash.to_owned(), format!("{stash}^2")] {
        let output = git.run(
            &[
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                &base,
                &side,
                "--",
            ],
            read_only(),
        )?;
        paths.extend(nul_list(&output.stdout));
    }
    let untracked = format!("{stash}^3");
    let (code, _) = git.run_status(
        &["rev-parse", "--verify", "--quiet", &untracked],
        read_only(),
    )?;
    if code == Some(0) {
        let output = git.run(
            &["ls-tree", "-r", "--name-only", "-z", &untracked],
            read_only(),
        )?;
        paths.extend(nul_list(&output.stdout));
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn capture_all(git: &dyn GitWriter, root: &Path, paths: &[String]) -> Result<Vec<RawFile>> {
    paths
        .iter()
        .map(|path| capture_file(git, root, path))
        .collect()
}

/// Runs `git stash pop` (`pop`) or `git stash apply` for the stash with
/// commit `id`. A command that stops with conflicted files is
/// [`StashOutcome::Conflicts`] (Git keeps the stash); any other failure is an
/// error. Refuses to start while the index has conflicts: Git cannot apply
/// a stash then (it reports "needs merge" / "could not write index").
pub fn apply_stash(git: &dyn GitWriter, id: &str, pop: bool) -> Result<StashOutcome> {
    if !unmerged_paths(git)?.is_empty() {
        return Err(Error::Backend(
            "the working copy has conflicted files; resolve or discard them first".into(),
        ));
    }
    let entries = stash_entries(git)?;
    let position = entries
        .iter()
        .position(|(stash, _)| stash == id)
        .ok_or_else(|| Error::NotFound(format!("stash {id}")))?;
    let name = format!("stash@{{{position}}}");
    let command = if pop { "pop" } else { "apply" };
    let args = ["stash", command, "--quiet", name.as_str()];
    let (code, output) = git.run_status(&args, RunOptions::default())?;
    if code == Some(0) {
        return Ok(StashOutcome::Applied);
    }
    if !unmerged_paths(git)?.is_empty() {
        return Ok(StashOutcome::Conflicts);
    }
    Err(Error::Command {
        command: format!("git {}", args.join(" ")),
        code,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

/// Like [`apply_stash`] but records what is needed to undo it.
pub fn apply_stash_recorded(
    git: &dyn GitWriter,
    root: &Path,
    id: &str,
    pop: bool,
) -> Result<StashRecord> {
    if !unmerged_paths(git)?.is_empty() {
        return Err(Error::Backend(
            "the working copy has conflicted files; resolve or discard them first".into(),
        ));
    }
    let entries = stash_entries(git)?;
    let (position, message) = entries
        .iter()
        .enumerate()
        .find(|(_, (stash, _))| stash == id)
        .map(|(position, (_, message))| (position, message.clone()))
        .ok_or_else(|| Error::NotFound(format!("stash {id}")))?;
    let head = head_commit(git)?
        .ok_or_else(|| Error::Backend("there is no commit to apply the stash on".into()))?;
    let index_tree = git
        .run(&["write-tree"], RunOptions::default())?
        .stdout_text()
        .trim()
        .to_owned();
    let paths = stash_paths(git, id)?;
    let before = capture_all(git, root, &paths)?;
    let outcome = apply_stash(git, id, pop)?;
    let after = capture_all(git, root, &paths)?;
    let dropped = !stash_entries(git)?.iter().any(|(stash, _)| stash == id);
    Ok(StashRecord {
        pop,
        stash: id.to_owned(),
        message,
        position,
        head,
        index_tree,
        before,
        after,
        outcome,
        dropped,
    })
}

/// True if a file the stash touched changed after the command (e.g. a
/// conflict was partly resolved): undoing then discards that work, so the
/// user is asked first.
pub fn stash_changed_since(git: &dyn GitWriter, root: &Path, record: &StashRecord) -> Result<bool> {
    for file in &record.after {
        if capture_file(git, root, &file.path)? != *file {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Returns the repository to the state before the pop or apply of
/// `record`: the index (also clearing unmerged entries), the touched files
/// and, if the command dropped it, the stash entry at its old position.
/// Fails without changes if HEAD moved since.
pub fn undo_stash(git: &dyn GitWriter, root: &Path, record: &StashRecord) -> Result<()> {
    if head_commit(git)?.as_deref() != Some(record.head.as_str()) {
        return Err(Error::Backend("Cannot undo: the repository changed".into()));
    }
    git.run(&["read-tree", &record.index_tree], RunOptions::default())?;
    for file in &record.before {
        restore_file(git, root, file)?;
    }
    if record.dropped {
        restore_entry(git, record)?;
    }
    Ok(())
}

/// Puts the dropped entry back at its old position: newer entries are
/// dropped and stored again on top of it (`git stash store`), so the list
/// reads exactly as before.
fn restore_entry(git: &dyn GitWriter, record: &StashRecord) -> Result<()> {
    let entries = stash_entries(git)?;
    let newer: Vec<(Oid, String)> = entries.into_iter().take(record.position).collect();
    for _ in &newer {
        git.run(
            &["stash", "drop", "--quiet", "stash@{0}"],
            RunOptions::default(),
        )?;
    }
    store(git, &record.stash, &record.message)?;
    for (stash, message) in newer.iter().rev() {
        store(git, stash, message)?;
    }
    Ok(())
}

fn store(git: &dyn GitWriter, stash: &str, message: &str) -> Result<()> {
    git.run(
        &["stash", "store", "--quiet", "-m", message, stash],
        RunOptions::default(),
    )
    .map(drop)
}

/// Pops or applies the stash of `record` again (after an undo). Refuses
/// when the stash is no longer in the list.
pub fn redo_stash(git: &dyn GitWriter, root: &Path, record: &StashRecord) -> Result<StashRecord> {
    if !stash_entries(git)?
        .iter()
        .any(|(stash, _)| *stash == record.stash)
    {
        return Err(Error::Backend(
            "Cannot redo: the stash is no longer in the list".into(),
        ));
    }
    apply_stash_recorded(git, root, &record.stash, record.pop)
}
