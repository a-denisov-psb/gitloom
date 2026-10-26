//! Reword and squash shortcuts (M4-08) on top of the interactive rebase
//! (M4-07): no panel, the plan is built here. Blocking: runs `git`.

use crate::merge::{rebase_commits, rebase_interactive, MergeOutcome};
use crate::todo::{build, TodoAction, TodoEntry};
use crate::{head_commit, Error, GitWriter, Result, RunOptions};

const NOT_CONTIGUOUS: &str =
    "Select commits that follow each other on the current branch, without merges";

/// Changes the message of commit `id`: HEAD is amended (staged changes are
/// left alone); an older commit of the current branch is reworded by an
/// interactive rebase from its parent.
pub fn reword_commit(
    git: &dyn GitWriter,
    id: &str,
    message: &str,
    autostash: bool,
) -> Result<MergeOutcome> {
    if head_commit(git)?.as_deref() == Some(id) {
        // `--only` without paths keeps the index out of the amended commit.
        git.run(
            &["commit", "--amend", "--only", "--allow-empty", "--file=-"],
            RunOptions {
                stdin: Some(message.as_bytes()),
                read_only: false,
            },
        )?;
        return Ok(MergeOutcome::Done);
    }
    let base = format!("{id}^");
    let mut entries = entries(git, &base)?;
    match entries.first_mut() {
        Some(first) if first.id == id => first.action = TodoAction::Reword,
        _ => return Err(Error::Backend(NOT_CONTIGUOUS.into())),
    }
    let input = build(&entries, |_| message.to_owned()).map_err(Error::Backend)?;
    rebase_interactive(git, &base, &input, autostash)
}

/// Squashes the commits `ids` (in any order) into one with `message`; they
/// must follow each other on the current branch. Later commits are kept.
pub fn squash_commits(
    git: &dyn GitWriter,
    ids: &[String],
    message: &str,
    autostash: bool,
) -> Result<MergeOutcome> {
    if ids.len() < 2 {
        return Err(Error::Backend(
            "Select at least two commits to squash".into(),
        ));
    }
    // The oldest selected commit is the one that is no descendant of any
    // other; its parent is the base.
    let oldest = oldest_of(git, ids)?;
    let base = format!("{oldest}^");
    let mut entries = entries(git, &base)?;
    let selected = entries
        .iter()
        .take(ids.len())
        .filter(|e| ids.contains(&e.id))
        .count();
    if selected != ids.len() {
        return Err(Error::Backend(NOT_CONTIGUOUS.into()));
    }
    for entry in entries.iter_mut().take(ids.len()).skip(1) {
        entry.action = TodoAction::Squash;
    }
    let input = build(&entries, |_| message.to_owned()).map_err(Error::Backend)?;
    rebase_interactive(git, &base, &input, autostash)
}

/// Commits `base..HEAD` as todo entries; fails unless that history is
/// linear (a merge in the range would be flattened by the rebase).
fn entries(git: &dyn GitWriter, base: &str) -> Result<Vec<TodoEntry>> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let range = format!("{base}..HEAD");
    let merges = git.run(&["rev-list", "--merges", "--count", &range], read)?;
    if merges.stdout_text().trim() != "0" {
        return Err(Error::Backend(NOT_CONTIGUOUS.into()));
    }
    Ok(rebase_commits(git, base)?
        .into_iter()
        .map(|(id, message)| TodoEntry {
            id,
            action: TodoAction::Pick,
            message,
        })
        .collect())
}

/// The selected commit whose parent is not selected (exactly one for a
/// contiguous run).
fn oldest_of<'a>(git: &dyn GitWriter, ids: &'a [String]) -> Result<&'a str> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let mut args = vec!["rev-list", "--no-walk=unsorted", "--parents"];
    args.extend(ids.iter().map(String::as_str));
    let output = git.run(&args, read)?;
    let text = output.stdout_text();
    let mut oldest = text.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let id = fields.next()?;
        let parent = fields.next();
        (!parent.is_some_and(|p| ids.iter().any(|i| i == p))).then_some(id)
    });
    match (oldest.next(), oldest.next()) {
        (Some(id), None) => ids
            .iter()
            .find(|i| i.as_str() == id)
            .map(String::as_str)
            .ok_or_else(|| Error::Backend(NOT_CONTIGUOUS.into())),
        _ => Err(Error::Backend(NOT_CONTIGUOUS.into())),
    }
}
