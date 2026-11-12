//! Undo and redo of local working copy actions (M12-07, ADR-043): stage
//! and unstage (index entries of the affected paths, or the whole index),
//! mark resolved (index entries and file content), stash push and stash
//! drop. Every undo and redo checks that the repository is still as the
//! action (or the undo) left it.
//!
//! Blocking: runs `git` and reads/writes files.

use std::path::Path;

use crate::discard::{index_snapshot, write_index_snapshot};
use crate::reset::{capture_file, restore_file};
use crate::stash::{stash_entries, store};
use crate::{head_commit, Error, GitWriter, IndexSnapshot, Oid, RawFile, Result, RunOptions};

/// Index state of a [`WorkChange`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexState {
    /// Entries of the affected paths (stage, unstage, lines, mark resolved).
    Paths(Vec<IndexSnapshot>),
    /// The whole index ("Stage all changes", "Unstage all changes").
    Whole {
        tree: Oid,
        unmerged: Vec<IndexSnapshot>,
    },
}

/// What a stage, unstage or mark resolved action changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkChange {
    pub label: String,
    pub index_before: IndexState,
    pub index_after: IndexState,
    /// Files before and after (mark resolved writes the resolution); empty
    /// for stage and unstage.
    pub files_before: Vec<RawFile>,
    pub files_after: Vec<RawFile>,
}

/// Which index part an action records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Paths(Vec<String>),
    Whole,
}

fn read_index(git: &dyn GitWriter, scope: &Scope) -> Result<IndexState> {
    Ok(match scope {
        Scope::Paths(paths) => IndexState::Paths(index_snapshot(git, paths)?),
        Scope::Whole => {
            let (tree, unmerged) = crate::record_index(git)?;
            IndexState::Whole { tree, unmerged }
        }
    })
}

fn write_index(git: &dyn GitWriter, state: &IndexState) -> Result<()> {
    match state {
        IndexState::Paths(entries) => write_index_snapshot(git, entries),
        IndexState::Whole { tree, unmerged } => crate::restore_index(git, tree, unmerged),
    }
}

fn scope_of(state: &IndexState) -> Scope {
    match state {
        IndexState::Paths(entries) => {
            Scope::Paths(entries.iter().map(|e| e.path.clone()).collect())
        }
        IndexState::Whole { .. } => Scope::Whole,
    }
}

fn capture_all(git: &dyn GitWriter, root: &Path, paths: &[String]) -> Result<Vec<RawFile>> {
    paths.iter().map(|p| capture_file(git, root, p)).collect()
}

/// Runs `op` and records the index of `scope` (and the content of `files`)
/// before and after it; `None` when nothing changed.
pub fn record_work_change(
    git: &dyn GitWriter,
    root: &Path,
    label: &str,
    scope: Scope,
    files: &[String],
    op: impl FnOnce(&dyn GitWriter) -> Result<()>,
) -> Result<Option<WorkChange>> {
    let index_before = read_index(git, &scope)?;
    let files_before = capture_all(git, root, files)?;
    op(git)?;
    let index_after = read_index(git, &scope)?;
    let files_after = capture_all(git, root, files)?;
    if index_before == index_after && files_before == files_after {
        return Ok(None);
    }
    Ok(Some(WorkChange {
        label: label.to_owned(),
        index_before,
        index_after,
        files_before,
        files_after,
    }))
}

/// Undoes (`undo`) or redoes `change`; refuses without changes when the
/// index entries or files are no longer as expected.
pub fn switch_work_change(
    git: &dyn GitWriter,
    root: &Path,
    change: &WorkChange,
    undo: bool,
) -> Result<()> {
    let (expected_index, expected_files, index, files) = if undo {
        (
            &change.index_after,
            &change.files_after,
            &change.index_before,
            &change.files_before,
        )
    } else {
        (
            &change.index_before,
            &change.files_before,
            &change.index_after,
            &change.files_after,
        )
    };
    let current = read_index(git, &scope_of(expected_index))?;
    let paths: Vec<String> = expected_files.iter().map(|f| f.path.clone()).collect();
    let current_files = capture_all(git, root, &paths)?;
    if current != *expected_index || current_files != *expected_files {
        return Err(changed(undo));
    }
    for file in files {
        restore_file(git, root, file)?;
    }
    write_index(git, index)
}

fn changed(undo: bool) -> Error {
    Error::Backend(format!(
        "Cannot {}: the repository changed",
        if undo { "undo" } else { "redo" }
    ))
}

/// A stash push: the entry, and the state right after it (precondition of
/// undo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashPushRecord {
    /// The message given to `git stash push`, if any (for redo).
    pub given: Option<String>,
    pub stash: Oid,
    pub head: Oid,
    /// `git status --porcelain` right after the push.
    pub status_after: String,
}

fn status(git: &dyn GitWriter) -> Result<String> {
    Ok(git
        .run(
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            RunOptions {
                read_only: true,
                ..RunOptions::default()
            },
        )?
        .stdout_text())
}

/// `git stash push --include-untracked` with `message`, recorded for undo.
pub fn stash_push_recorded(git: &dyn GitWriter, message: Option<&str>) -> Result<StashPushRecord> {
    let before = stash_entries(git)?.first().map(|(id, _)| id.clone());
    git.stash_push(message)?;
    let stash = stash_entries(git)?
        .first()
        .map(|(id, _)| id.clone())
        .filter(|id| Some(id) != before.as_ref())
        .ok_or_else(|| Error::Backend("there were no changes to stash".into()))?;
    let head = head_commit(git)?.ok_or_else(|| Error::Backend("no commit to stash on".into()))?;
    Ok(StashPushRecord {
        given: message.map(str::to_owned),
        stash,
        head,
        status_after: status(git)?,
    })
}

/// Applies the pushed stash with `--index` and drops it, only while HEAD,
/// index and working tree are as the push left them.
pub fn undo_stash_push(git: &dyn GitWriter, record: &StashPushRecord) -> Result<()> {
    let position = stash_entries(git)?
        .iter()
        .position(|(id, _)| *id == record.stash)
        .ok_or_else(|| Error::Backend("Cannot undo: the stash is no longer in the list".into()))?;
    if head_commit(git)?.as_deref() != Some(record.head.as_str())
        || status(git)? != record.status_after
    {
        return Err(changed(true));
    }
    let name = format!("stash@{{{position}}}");
    git.run(
        &["stash", "apply", "--index", "--quiet", &name],
        RunOptions::default(),
    )?;
    git.run(&["stash", "drop", "--quiet", &name], RunOptions::default())
        .map(drop)
}

/// Stashes again after an undo; the new entry replaces the record.
pub fn redo_stash_push(git: &dyn GitWriter, record: &StashPushRecord) -> Result<StashPushRecord> {
    stash_push_recorded(git, record.given.as_deref())
}

/// A deleted stash entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashDropRecord {
    pub stash: Oid,
    pub message: String,
    /// Position in the list before the drop.
    pub position: usize,
}

/// Drops the stash with commit `id`, recorded for undo.
pub fn stash_drop_recorded(git: &dyn GitWriter, id: &str) -> Result<StashDropRecord> {
    let entries = stash_entries(git)?;
    let (position, message) = entries
        .iter()
        .enumerate()
        .find(|(_, (stash, _))| stash == id)
        .map(|(position, (_, message))| (position, message.clone()))
        .ok_or_else(|| Error::NotFound(format!("stash {id}")))?;
    git.run(
        &["stash", "drop", "--quiet", &format!("stash@{{{position}}}")],
        RunOptions::default(),
    )?;
    Ok(StashDropRecord {
        stash: id.to_owned(),
        message,
        position,
    })
}

/// Stores the dropped entry back at its old position (newer entries are
/// dropped and stored again on top).
pub fn undo_stash_drop(git: &dyn GitWriter, record: &StashDropRecord) -> Result<()> {
    let entries = stash_entries(git)?;
    if entries.iter().any(|(id, _)| *id == record.stash) {
        return Err(changed(true));
    }
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

/// Drops the entry again after an undo.
pub fn redo_stash_drop(git: &dyn GitWriter, record: &StashDropRecord) -> Result<StashDropRecord> {
    stash_drop_recorded(git, &record.stash)
}

/// A renamed stash (M12-10): the entries from `stash@{0}` to the renamed
/// one before and after, `(commit, message)` each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashRename {
    pub before: Vec<(Oid, String)>,
    pub after: Vec<(Oid, String)>,
}

/// New message for a stash renamed to `name`: `On <branch>: <name>` with
/// the branch of the old message ("WIP on <branch>: …" or "On <branch>:
/// …"), else `name` alone.
pub fn renamed_message(old: &str, name: &str) -> String {
    let branch = old
        .strip_prefix("WIP on ")
        .or_else(|| old.strip_prefix("On "))
        .and_then(|rest| rest.split_once(':'))
        .map(|(branch, _)| branch);
    match branch {
        Some(branch) => format!("On {branch}: {name}"),
        None => name.to_owned(),
    }
}

/// A copy of commit `id` with `message`: same tree, parents, author and
/// committer (a signature is left out, it would no longer match).
fn reword_commit(git: &dyn GitWriter, id: &str, message: &str) -> Result<Oid> {
    let raw = git
        .run(
            &["cat-file", "commit", id],
            RunOptions {
                read_only: true,
                ..RunOptions::default()
            },
        )?
        .stdout_text();
    let headers = raw.split_once("\n\n").map_or(raw.as_str(), |(h, _)| h);
    let mut kept = Vec::new();
    let mut in_signature = false;
    for line in headers.lines() {
        if line.starts_with("gpgsig") {
            in_signature = true;
            continue;
        }
        if in_signature && line.starts_with(' ') {
            continue;
        }
        in_signature = false;
        kept.push(line);
    }
    let content = format!("{}\n\n{}\n", kept.join("\n"), message.trim_end());
    Ok(git
        .run(
            &["hash-object", "-t", "commit", "-w", "--stdin"],
            RunOptions {
                stdin: Some(content.as_bytes()),
                read_only: false,
            },
        )?
        .stdout_text()
        .trim()
        .to_owned())
}

/// Replaces the top entries `from` (`stash@{0}` …) by `to`; on a failure
/// the original entries are stored back.
fn replace_top(git: &dyn GitWriter, from: &[(Oid, String)], to: &[(Oid, String)]) -> Result<()> {
    let attempt = || -> Result<()> {
        for _ in from {
            git.run(
                &["stash", "drop", "--quiet", "stash@{0}"],
                RunOptions::default(),
            )?;
        }
        for (stash, message) in to.iter().rev() {
            store(git, stash, message)?;
        }
        Ok(())
    };
    let result = attempt();
    if result.is_err() {
        // Back to the recorded entries: drop what is above the untouched
        // rest of the list, then store them again.
        let rest = stash_entries(git)?;
        let extra = rest
            .iter()
            .take_while(|(id, _)| from.iter().chain(to).any(|(o, _)| o == id))
            .count();
        for _ in 0..extra {
            git.run(
                &["stash", "drop", "--quiet", "stash@{0}"],
                RunOptions::default(),
            )?;
        }
        for (stash, message) in from.iter().rev() {
            store(git, stash, message)?;
        }
    }
    result
}

/// Renames the stash with commit `id` to `name` at its position.
pub fn rename_stash(git: &dyn GitWriter, id: &str, name: &str) -> Result<StashRename> {
    let entries = stash_entries(git)?;
    let position = entries
        .iter()
        .position(|(stash, _)| stash == id)
        .ok_or_else(|| Error::NotFound(format!("stash {id}")))?;
    let before: Vec<(Oid, String)> = entries.into_iter().take(position + 1).collect();
    let message = renamed_message(&before[position].1, name);
    let renamed = reword_commit(git, id, &message)?;
    let mut after = before.clone();
    after[position] = (renamed, message);
    replace_top(git, &before, &after)?;
    Ok(StashRename { before, after })
}

/// Undoes (`undo`) or redoes a rename; refuses when the entries up to the
/// renamed one are no longer as the rename (or the undo) left them.
pub fn switch_stash_rename(git: &dyn GitWriter, rename: &StashRename, undo: bool) -> Result<()> {
    let (expected, target) = if undo {
        (&rename.after, &rename.before)
    } else {
        (&rename.before, &rename.after)
    };
    let current: Vec<Oid> = stash_entries(git)?
        .into_iter()
        .take(expected.len())
        .map(|(id, _)| id)
        .collect();
    let wanted: Vec<&Oid> = expected.iter().map(|(id, _)| id).collect();
    if current.iter().collect::<Vec<_>>() != wanted {
        return Err(Error::Backend(format!(
            "Cannot {}: the stash list changed",
            if undo { "undo" } else { "redo" }
        )));
    }
    replace_top(git, expected, target)
}
