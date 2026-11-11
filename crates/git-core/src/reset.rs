//! Reset (M4-10) and the snapshot that makes a hard reset undoable
//! (ADR-014 addendum). Blocking: runs `git` and reads/writes files.

use std::path::Path;

use crate::discard::is_executable;
use crate::history::set_executable;
use crate::{head_commit, Error, GitWriter, Oid, Result, RunOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    /// Keep index and working tree: the changes of the dropped commits end
    /// up staged.
    Soft,
    /// Reset the index, keep the working tree: changes end up unstaged.
    Mixed,
    /// Reset index and working tree: uncommitted changes are discarded
    /// (untracked files stay).
    Hard,
}

impl ResetMode {
    fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Soft => "Soft",
            Self::Mixed => "Mixed",
            Self::Hard => "Hard",
        }
    }
}

/// Moves HEAD (its branch) to `id` (`git reset --<mode>`).
pub fn reset(git: &dyn GitWriter, id: &str, mode: ResetMode) -> Result<()> {
    git.run(
        &["reset", "--quiet", mode.flag(), id, "--"],
        RunOptions::default(),
    )
    .map(drop)
}

/// Working tree content of one path before a hard reset, stored as a blob
/// without filters (byte for byte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFile {
    pub path: String,
    /// `None` if the file did not exist.
    pub blob: Option<Oid>,
    pub executable: bool,
}

/// Everything a hard reset changed: HEAD, the index (as a tree) and the
/// files it overwrote or deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetSnapshot {
    pub head_before: Oid,
    pub head_after: Oid,
    pub index_tree: Oid,
    pub files: Vec<RawFile>,
}

fn read_only() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

fn nul_list(output: &crate::GitOutput) -> Vec<String> {
    output
        .stdout
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// Paths a hard reset to `id` may overwrite or delete: tracked paths that
/// differ from `id`, and untracked files that `id` contains.
fn affected_paths(git: &dyn GitWriter, id: &str) -> Result<Vec<String>> {
    let mut paths = nul_list(&git.run(
        &["diff", "--name-only", "--no-renames", "-z", id, "--"],
        read_only(),
    )?);
    let untracked = nul_list(&git.run(
        &["ls-files", "--others", "--exclude-standard", "-z"],
        read_only(),
    )?);
    if !untracked.is_empty() {
        let target = nul_list(&git.run(&["ls-tree", "-r", "--name-only", "-z", id], read_only())?);
        paths.extend(untracked.into_iter().filter(|p| target.contains(p)));
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Hard reset to `id` that first records HEAD, the index and every file the
/// reset touches, so [`undo_hard_reset`] can bring them back.
pub fn hard_reset_with_snapshot(
    git: &dyn GitWriter,
    root: &Path,
    id: &str,
) -> Result<ResetSnapshot> {
    let head_before =
        head_commit(git)?.ok_or_else(|| Error::Backend("There is no commit to reset".into()))?;
    let index_tree = git
        .run(&["write-tree"], RunOptions::default())?
        .stdout_text();
    let mut files = Vec::new();
    for path in affected_paths(git, id)? {
        files.push(capture_file(git, root, &path)?);
    }
    reset(git, id, ResetMode::Hard)?;
    let head_after = head_commit(git)?.unwrap_or_default();
    Ok(ResetSnapshot {
        head_before,
        head_after,
        index_tree: index_tree.trim().to_owned(),
        files,
    })
}

/// Restores HEAD, index and files recorded by [`hard_reset_with_snapshot`].
/// Fails without changes if HEAD moved since.
pub fn undo_hard_reset(git: &dyn GitWriter, root: &Path, snapshot: &ResetSnapshot) -> Result<()> {
    if head_commit(git)?.as_deref() != Some(snapshot.head_after.as_str()) {
        return Err(Error::Backend("Cannot undo: the repository changed".into()));
    }
    reset(git, &snapshot.head_before, ResetMode::Soft)?;
    git.run(&["read-tree", &snapshot.index_tree], RunOptions::default())?;
    for file in &snapshot.files {
        restore_file(git, root, file)?;
    }
    Ok(())
}

/// Records the working tree content of `path` (byte for byte, as a blob
/// without filters); `blob` is `None` if it is not a file.
pub(crate) fn capture_file(git: &dyn GitWriter, root: &Path, path: &str) -> Result<RawFile> {
    let full = root.join(path);
    let blob = if full.is_file() {
        let data = std::fs::read(&full)
            .map_err(|e| Error::Backend(format!("could not read {path}: {e}")))?;
        Some(git.write_blob(&data)?)
    } else {
        None
    };
    Ok(RawFile {
        executable: blob.is_some() && is_executable(&full),
        path: path.to_owned(),
        blob,
    })
}

/// Writes the content recorded by [`capture_file`] back, or deletes the
/// file if it did not exist (empty parent folders are removed as well).
pub(crate) fn restore_file(git: &dyn GitWriter, root: &Path, file: &RawFile) -> Result<()> {
    let full = root.join(&file.path);
    match &file.blob {
        Some(blob) => {
            let data = git.run(&["cat-file", "blob", blob], read_only())?.stdout;
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::Backend(format!("could not create {}: {e}", parent.display()))
                })?;
            }
            std::fs::write(&full, data)
                .map_err(|e| Error::Backend(format!("could not write {}: {e}", file.path)))?;
            set_executable(&full, file.executable)
        }
        None => {
            match std::fs::remove_file(&full) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(Error::Backend(format!(
                        "could not delete {}: {e}",
                        file.path
                    )))
                }
            }
            remove_empty_parents(root, &full);
            Ok(())
        }
    }
}

/// Removes folders left empty by a deleted file, up to (not including) `root`.
fn remove_empty_parents(root: &Path, file: &Path) {
    let mut dir = file.parent();
    while let Some(current) = dir {
        if current == root || !current.starts_with(root) {
            break;
        }
        // Fails (and stops) as soon as a folder is not empty.
        if std::fs::remove_dir(current).is_err() {
            break;
        }
        dir = current.parent();
    }
}
