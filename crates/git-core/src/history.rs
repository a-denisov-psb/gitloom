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
        let entries: Vec<(String, Option<(String, String)>)> = snapshot
            .index
            .iter()
            .map(|e| (e.path.clone(), e.entry.clone()))
            .collect();
        writer.set_index_entries(&entries)?;
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
fn set_executable(path: &Path, executable: bool) -> Result<()> {
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
fn set_executable(_path: &Path, _executable: bool) -> Result<()> {
    Ok(())
}
