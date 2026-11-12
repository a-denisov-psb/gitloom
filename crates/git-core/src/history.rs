//! Undo and redo primitives (ADR-014): moving HEAD with a precondition and
//! restoring working tree snapshots.
//!
//! Blocking: `git` processes and file system access.

use std::path::Path;

use crate::{DiscardSnapshot, Error, FileSnapshot, GitWriter, Result, RunOptions};

/// Object id meaning "must not exist" for `git update-ref`.
const NULL_ID: &str = "0000000000000000000000000000000000000000";

/// Current HEAD commit, `None` before the first commit.
pub fn head_commit(writer: &dyn GitWriter) -> Result<Option<String>> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    match writer.run(&["rev-parse", "--verify", "--quiet", "HEAD"], read) {
        Ok(output) => Ok(Some(output.stdout_text().trim().to_owned())),
        Err(Error::Command { code: Some(1), .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Moves HEAD (its branch, if any) from `from` to `to` without touching the
/// index or working tree, like `git reset --soft`. Fails with "the
/// repository changed" if HEAD is not at `from`. `None` means unborn.
pub fn move_head(
    writer: &dyn GitWriter,
    from: Option<&str>,
    to: Option<&str>,
    reason: &str,
) -> Result<()> {
    let changed = || Error::Backend("Cannot undo: the repository changed".into());
    if head_commit(writer)?.as_deref() != from {
        return Err(changed());
    }
    let old = from.unwrap_or(NULL_ID);
    let result = match to {
        Some(to) => writer.run(
            &["update-ref", "-m", reason, "HEAD", to, old],
            RunOptions::default(),
        ),
        None => writer.run(
            &["update-ref", "-m", reason, "-d", "HEAD", old],
            RunOptions::default(),
        ),
    };
    // A race between the check and the update still fails safely.
    // A lock held by another process is reported as is, so it can be retried.
    result.map(drop).map_err(|error| match error {
        Error::Command { ref stderr, .. } if crate::locked_file(stderr).is_none() => changed(),
        other => other,
    })
}

/// Writes `snapshot` back into the working tree and returns a snapshot of
/// what was there before, so the restore itself can be reverted.
pub fn restore_snapshot(
    writer: &dyn GitWriter,
    root: &Path,
    snapshot: &DiscardSnapshot,
) -> Result<DiscardSnapshot> {
    let paths: Vec<String> = snapshot.files.iter().map(|f| f.path.clone()).collect();
    let mut current = crate::discard::snapshot(writer, root, &paths)?;
    if !snapshot.index.is_empty() {
        let index_paths: Vec<String> = snapshot.index.iter().map(|e| e.path.clone()).collect();
        current.index = crate::discard::index_snapshot(writer, &index_paths)?;
        crate::discard::write_index_snapshot(writer, &snapshot.index)?;
    }
    for file in &snapshot.files {
        restore_file(writer, root, file)?;
    }
    Ok(current)
}

fn restore_file(writer: &dyn GitWriter, root: &Path, file: &FileSnapshot) -> Result<()> {
    let full = root.join(&file.path);
    let Some(blob) = &file.blob else {
        return match std::fs::remove_file(&full) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Backend(format!(
                "could not delete {}: {e}",
                file.path
            ))),
        };
    };
    let data = writer.blob_for_worktree(&file.path, blob)?;
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Backend(format!("could not create {}: {e}", parent.display())))?;
    }
    std::fs::write(&full, data)
        .map_err(|e| Error::Backend(format!("could not write {}: {e}", file.path)))?;
    set_executable(&full, file.executable)
}

#[cfg(unix)]
pub(crate) fn set_executable(path: &Path, executable: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let metadata = std::fs::metadata(path).map_err(|e| Error::Backend(e.to_string()))?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let new = if executable {
        mode | 0o111
    } else {
        mode & !0o111
    };
    if new != mode {
        permissions.set_mode(new);
        std::fs::set_permissions(path, permissions).map_err(|e| Error::Backend(e.to_string()))?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn set_executable(_path: &Path, _executable: bool) -> Result<()> {
    Ok(())
}

/// Where HEAD is: on a local branch or detached at a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadPosition {
    Branch(String),
    Detached(String),
}

impl HeadPosition {
    /// Branch name or short commit id, for labels.
    pub fn name(&self) -> String {
        match self {
            Self::Branch(name) => name.clone(),
            Self::Detached(id) => id[..id.len().min(7)].to_owned(),
        }
    }
}

fn read_only() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

/// Current HEAD position; `None` before the first commit.
pub fn head_position(writer: &dyn GitWriter) -> Result<Option<HeadPosition>> {
    match writer.run(&["symbolic-ref", "--quiet", "--short", "HEAD"], read_only()) {
        Ok(output) => {
            let branch = output.stdout_text().trim().to_owned();
            // An unborn branch has no position to return to.
            Ok(head_commit(writer)?.map(|_| HeadPosition::Branch(branch)))
        }
        Err(Error::Command { code: Some(1), .. }) => {
            Ok(head_commit(writer)?.map(HeadPosition::Detached))
        }
        Err(error) => Err(error),
    }
}

/// Commit `reference` (e.g. `refs/heads/main`, tags peeled) points to.
pub fn ref_commit(writer: &dyn GitWriter, reference: &str) -> Result<Option<String>> {
    let spec = format!("{reference}^{{commit}}");
    match writer.run(&["rev-parse", "--verify", "--quiet", &spec], read_only()) {
        Ok(output) => Ok(Some(output.stdout_text().trim().to_owned())),
        Err(Error::Command { code: Some(1), .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Commits reachable from HEAD but from no branch, remote branch or tag
/// (newest first, at most 100): leaving a detached HEAD would lose track of them.
pub fn unreachable_from_branches(writer: &dyn GitWriter) -> Result<Vec<String>> {
    if head_commit(writer)?.is_none() {
        return Ok(Vec::new());
    }
    let output = writer.run(
        &[
            "rev-list",
            "--max-count=100",
            "HEAD",
            "--not",
            "--branches",
            "--remotes",
            "--tags",
        ],
        read_only(),
    )?;
    Ok(output
        .stdout_text()
        .lines()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect())
}

/// Upstream of local branch `branch` (e.g. `origin/main`), if set.
pub fn branch_upstream(writer: &dyn GitWriter, branch: &str) -> Result<Option<String>> {
    let spec = format!("{branch}@{{upstream}}");
    match writer.run(&["rev-parse", "--abbrev-ref", &spec], read_only()) {
        Ok(output) => Ok(Some(output.stdout_text().trim().to_owned())),
        Err(Error::Command { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Message of annotated tag `name`; `None` for a lightweight tag.
pub fn tag_message(writer: &dyn GitWriter, name: &str) -> Result<Option<String>> {
    let full = format!("refs/tags/{name}");
    let kind = writer.run(&["cat-file", "-t", &full], read_only())?;
    if kind.stdout_text().trim() != "tag" {
        return Ok(None);
    }
    let format = "--format=%(contents)";
    let output = writer.run(&["tag", "--list", format, name], read_only())?;
    Ok(Some(output.stdout_text().trim_end().to_owned()))
}

/// An undoable change of branches, tags or HEAD (M3-12, ADR-014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefChange {
    Checkout {
        from: HeadPosition,
        to: HeadPosition,
    },
    /// Branch created at `target` and checked out, coming from `from`.
    CreateBranch {
        name: String,
        target: String,
        from: HeadPosition,
    },
    DeleteBranch {
        name: String,
        target: String,
        upstream: Option<String>,
    },
    RenameBranch {
        old: String,
        new: String,
    },
    CreateTag {
        name: String,
        target: String,
        message: Option<String>,
    },
    DeleteTag {
        name: String,
        target: String,
        message: Option<String>,
    },
}

impl RefChange {
    /// For the Undo/Redo tooltips.
    pub fn label(&self) -> String {
        match self {
            Self::Checkout { to, .. } => format!("Checkout {}", to.name()),
            Self::CreateBranch { name, .. } => format!("Create branch {name}"),
            Self::DeleteBranch { name, .. } => format!("Delete branch {name}"),
            Self::RenameBranch { old, new } => format!("Rename branch {old} → {new}"),
            Self::CreateTag { name, .. } => format!("Create tag {name}"),
            Self::DeleteTag { name, .. } => format!("Delete tag {name}"),
        }
    }

    /// Reverts the change (`undo`) or applies it again. Fails with "the
    /// repository changed" if the refs are no longer where the change left them.
    pub fn apply(&self, writer: &dyn GitWriter, undo: bool) -> Result<()> {
        let changed = || {
            Error::Backend(format!(
                "Cannot {}: the repository changed",
                if undo { "undo" } else { "redo" }
            ))
        };
        let expect = |ok: bool| if ok { Ok(()) } else { Err(changed()) };
        let branch = |name: &str| ref_commit(writer, &format!("refs/heads/{name}"));
        let tag = |name: &str| ref_commit(writer, &format!("refs/tags/{name}"));
        match self {
            Self::Checkout { from, to } => {
                let (expected, target) = if undo { (to, from) } else { (from, to) };
                expect(head_position(writer)?.as_ref() == Some(expected))?;
                switch_to(writer, target)
            }
            Self::CreateBranch { name, target, from } if undo => {
                expect(branch(name)?.as_deref() == Some(target.as_str()))?;
                if head_position(writer)? == Some(HeadPosition::Branch(name.clone())) {
                    switch_to(writer, from)?;
                }
                writer.delete_branch(name, true)
            }
            Self::CreateBranch { name, target, from } => {
                expect(branch(name)?.is_none())?;
                expect(head_position(writer)?.as_ref() == Some(from))?;
                writer.create_branch(name, target)
            }
            Self::DeleteBranch {
                name,
                target,
                upstream,
            } if undo => {
                expect(branch(name)?.is_none())?;
                writer.run(&["branch", name, target], RunOptions::default())?;
                match upstream {
                    Some(upstream) => writer.set_upstream(name, upstream),
                    None => Ok(()),
                }
            }
            Self::DeleteBranch { name, target, .. } => {
                expect(branch(name)?.as_deref() == Some(target.as_str()))?;
                writer.delete_branch(name, true)
            }
            Self::RenameBranch { old, new } => {
                let (from, to) = if undo { (new, old) } else { (old, new) };
                expect(branch(from)?.is_some() && branch(to)?.is_none())?;
                writer.rename_branch(from, to)
            }
            Self::CreateTag { name, target, .. } if undo => {
                expect(tag(name)?.as_deref() == Some(target.as_str()))?;
                writer.delete_tag(name)
            }
            Self::DeleteTag {
                name,
                target,
                message,
            } if undo => {
                expect(tag(name)?.is_none())?;
                writer.create_tag(name, target, message.as_deref())
            }
            Self::CreateTag {
                name,
                target,
                message,
            } => {
                expect(tag(name)?.is_none())?;
                writer.create_tag(name, target, message.as_deref())
            }
            Self::DeleteTag { name, target, .. } => {
                expect(tag(name)?.as_deref() == Some(target.as_str()))?;
                writer.delete_tag(name)
            }
        }
    }
}

fn switch_to(writer: &dyn GitWriter, position: &HeadPosition) -> Result<()> {
    match position {
        HeadPosition::Branch(name) => writer.switch_branch(name),
        HeadPosition::Detached(id) => writer.switch_detached(id),
    }
}

/// Result of deleting several local branches and tags (M12-11).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeleteOutcome {
    /// What was deleted, for one undo step.
    pub changes: Vec<RefChange>,
    /// Branches `git branch -d` refused because they are not merged.
    pub unmerged: Vec<String>,
    /// Other failures: (name, Git's message).
    pub failed: Vec<(String, String)>,
}

/// Deletes local `branches` (`-d`, or `-D` with `force`) and `tags` one by
/// one; a failure does not stop the others.
pub fn delete_refs(
    writer: &dyn GitWriter,
    branches: &[String],
    tags: &[String],
    force: bool,
) -> Result<DeleteOutcome> {
    let mut outcome = DeleteOutcome::default();
    for name in branches {
        let Some(target) = crate::ref_commit(writer, &format!("refs/heads/{name}"))? else {
            outcome
                .failed
                .push((name.clone(), "the branch no longer exists".into()));
            continue;
        };
        let upstream = branch_upstream(writer, name)?;
        match writer.delete_branch(name, force) {
            Ok(()) => outcome.changes.push(RefChange::DeleteBranch {
                name: name.clone(),
                target,
                upstream,
            }),
            Err(error) if crate::branch_not_merged(&error.to_string()) => {
                outcome.unmerged.push(name.clone());
            }
            Err(error) => outcome.failed.push((name.clone(), error.to_string())),
        }
    }
    for name in tags {
        let Some(target) = crate::ref_commit(writer, &format!("refs/tags/{name}"))? else {
            outcome
                .failed
                .push((name.clone(), "the tag no longer exists".into()));
            continue;
        };
        let message = tag_message(writer, name)?;
        match writer.delete_tag(name) {
            Ok(()) => outcome.changes.push(RefChange::DeleteTag {
                name: name.clone(),
                target,
                message,
            }),
            Err(error) => outcome.failed.push((name.clone(), error.to_string())),
        }
    }
    Ok(outcome)
}
