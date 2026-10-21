//! Discarding working tree changes, with snapshots for undo (ADR-014).
//!
//! Blocking: file system access and `git` processes. Call off the UI thread.

use std::path::Path;

use crate::{ChangeKind, DiffLineKind, Error, FileChange, FileDiff, GitWriter, Result};

/// Working tree state of one file before a discard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshot {
    pub path: String,
    /// Blob with the file content (written with filters, like `git add`);
    /// `None` if the file did not exist.
    pub blob: Option<String>,
    pub executable: bool,
}

/// Everything needed to undo a discard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiscardSnapshot {
    pub files: Vec<FileSnapshot>,
}

/// Records the current content of `paths` as blobs.
pub(crate) fn snapshot(
    writer: &dyn GitWriter,
    root: &Path,
    paths: &[String],
) -> Result<DiscardSnapshot> {
    let existing: Vec<String> = paths
        .iter()
        .filter(|p| root.join(p).is_file())
        .cloned()
        .collect();
    let blobs = writer.write_worktree_blobs(&existing)?;
    if blobs.len() != existing.len() {
        return Err(Error::Backend(
            "could not snapshot the files to discard".into(),
        ));
    }
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let blob = existing
            .iter()
            .position(|p| p == path)
            .and_then(|i| blobs.get(i).cloned());
        files.push(FileSnapshot {
            executable: is_executable(&root.join(path)),
            path: path.clone(),
            blob,
        });
    }
    Ok(DiscardSnapshot { files })
}

/// Discards the working tree changes of `files`: tracked files go back to
/// their index content, untracked files are deleted.
pub fn discard_files(
    writer: &dyn GitWriter,
    root: &Path,
    files: &[FileChange],
) -> Result<DiscardSnapshot> {
    let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let snapshot = snapshot(writer, root, &paths)?;
    let (untracked, tracked): (Vec<&FileChange>, Vec<&FileChange>) =
        files.iter().partition(|f| f.kind == ChangeKind::Untracked);
    let tracked: Vec<String> = tracked.iter().map(|f| f.path.clone()).collect();
    writer.restore_worktree(&tracked)?;
    for file in untracked {
        remove_file(root, &file.path)?;
    }
    Ok(snapshot)
}

/// Discards all unstaged changes and untracked files listed in `unstaged`.
/// Staged changes stay in the index.
pub fn discard_all(
    writer: &dyn GitWriter,
    root: &Path,
    unstaged: &[FileChange],
) -> Result<DiscardSnapshot> {
    discard_files(writer, root, unstaged)
}

/// Reverts the selected changed lines of an unstaged (or untracked) diff in
/// the working tree. `content` is the current working tree content in Git
/// form (filters applied), as the diff was computed from it.
pub fn discard_lines(
    writer: &dyn GitWriter,
    root: &Path,
    diff: &FileDiff,
    content: &[u8],
    selected: impl Fn(usize, usize) -> bool,
) -> Result<DiscardSnapshot> {
    let path = diff
        .new_path
        .clone()
        .ok_or_else(|| Error::Backend("the file no longer exists".into()))?;
    let snapshot = snapshot(writer, root, std::slice::from_ref(&path))?;
    let reverted = revert_lines(content, diff, selected);
    // Back to working tree form (eol, smudge filters) through Git itself.
    let blob = writer.write_blob(&reverted)?;
    let data = writer.blob_for_worktree(&path, &blob)?;
    std::fs::write(root.join(&path), data)
        .map_err(|e| Error::Backend(format!("could not write {path}: {e}")))?;
    Ok(snapshot)
}

/// `content` (the new side of `diff`) with the selected changes undone:
/// selected added lines are removed and selected removed lines come back.
pub fn revert_lines(
    content: &[u8],
    diff: &FileDiff,
    selected: impl Fn(usize, usize) -> bool,
) -> Vec<u8> {
    let lines: Vec<&[u8]> = content.split_inclusive(|&b| b == b'\n').collect();
    let mut out = Vec::with_capacity(content.len());
    let mut cursor = 0usize;
    for (h, hunk) in diff.hunks.iter().enumerate() {
        let first = if hunk.new_count == 0 {
            hunk.new_start as usize
        } else {
            hunk.new_start as usize - 1
        };
        while cursor < first && cursor < lines.len() {
            out.extend_from_slice(lines[cursor]);
            cursor += 1;
        }
        for (l, line) in hunk.lines.iter().enumerate() {
            let pick = selected(h, l);
            match line.kind {
                DiffLineKind::Context => {
                    if let Some(original) = lines.get(cursor) {
                        out.extend_from_slice(original);
                    }
                    cursor += 1;
                }
                DiffLineKind::Added => {
                    if !pick {
                        if let Some(original) = lines.get(cursor) {
                            out.extend_from_slice(original);
                        }
                    }
                    cursor += 1;
                }
                DiffLineKind::Removed => {
                    if pick {
                        out.extend_from_slice(line.content.as_bytes());
                        if !line.missing_newline {
                            out.extend_from_slice(if line.crlf { b"\r\n" } else { b"\n" });
                        }
                    }
                }
            }
        }
    }
    while cursor < lines.len() {
        out.extend_from_slice(lines[cursor]);
        cursor += 1;
    }
    out
}

fn remove_file(root: &Path, path: &str) -> Result<()> {
    let full = root.join(path);
    let result = if full.is_dir() {
        std::fs::remove_dir_all(&full)
    } else {
        std::fs::remove_file(&full)
    };
    match result {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Backend(format!("could not delete {path}: {e}"))),
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::text_diff;

    fn file_diff(old: &str, new: &str) -> FileDiff {
        FileDiff {
            old_path: Some("f".into()),
            new_path: Some("f".into()),
            binary: false,
            too_large: false,
            hunks: text_diff(old.as_bytes(), new.as_bytes()).hunks,
        }
    }

    #[test]
    fn reverting_everything_gives_the_old_content() {
        let old = "a\nb\nc\nd\n";
        let new = "a\nB\nc\nd\ne\n";
        let diff = file_diff(old, new);
        assert_eq!(
            revert_lines(new.as_bytes(), &diff, |_, _| true),
            old.as_bytes()
        );
        assert_eq!(
            revert_lines(new.as_bytes(), &diff, |_, _| false),
            new.as_bytes()
        );
    }

    #[test]
    fn reverting_one_added_line() {
        let old = "a\nb\n";
        let new = "a\nx\nb\ny\n";
        let diff = file_diff(old, new);
        // Hunk lines: " a", "+x", " b", "+y" → revert only "+y".
        let reverted = revert_lines(new.as_bytes(), &diff, |_, l| l == 3);
        assert_eq!(reverted, b"a\nx\nb\n");
    }

    #[test]
    fn reverting_keeps_crlf_and_missing_newline() {
        let old = "a\r\nb";
        let new = "a\r\nB";
        let diff = file_diff(old, new);
        assert_eq!(
            revert_lines(new.as_bytes(), &diff, |_, _| true),
            old.as_bytes()
        );
    }
}
