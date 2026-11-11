//! Discarding working tree changes, with snapshots for undo (ADR-014).
//!
//! Blocking: file system access and `git` processes. Call off the UI thread.

use std::path::Path;

use std::collections::HashMap;

use crate::{
    ChangeKind, DiffLineKind, Error, FileChange, FileDiff, GitWriter, Result, WorkingTree,
};

/// Working tree state of one file before a discard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshot {
    pub path: String,
    /// Blob with the file content (written with filters, like `git add`);
    /// `None` if the file did not exist.
    pub blob: Option<String>,
    pub executable: bool,
}

/// Index entry of one path before a discard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSnapshot {
    pub path: String,
    /// `(mode, object id)`; `None` if the path was not in the index.
    pub entry: Option<(String, String)>,
    /// Unmerged entries `(mode, object id, stage 1–3)` of a conflicted path
    /// (M9-03); empty otherwise.
    pub stages: Vec<(String, String, u8)>,
}

/// Everything needed to undo a discard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiscardSnapshot {
    pub files: Vec<FileSnapshot>,
    /// Index entries, only for discards that changed the index.
    pub index: Vec<IndexSnapshot>,
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
    Ok(DiscardSnapshot {
        files,
        index: Vec::new(),
    })
}

/// Records the index entries of `paths`, unmerged ones included.
pub(crate) fn index_snapshot(
    writer: &dyn GitWriter,
    paths: &[String],
) -> Result<Vec<IndexSnapshot>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut entries: HashMap<String, IndexSnapshot> = HashMap::new();
    for (path, mode, id, stage) in all_index_entries(writer)? {
        let entry = entries
            .entry(path.clone())
            .or_insert_with(|| IndexSnapshot {
                path,
                entry: None,
                stages: Vec::new(),
            });
        if stage == 0 {
            entry.entry = Some((mode, id));
        } else {
            entry.stages.push((mode, id, stage));
        }
    }
    Ok(paths
        .iter()
        .map(|path| {
            entries.remove(path).unwrap_or_else(|| IndexSnapshot {
                path: path.clone(),
                entry: None,
                stages: Vec::new(),
            })
        })
        .collect())
}

/// All index entries as `(path, mode, object id, stage)`.
fn all_index_entries(writer: &dyn GitWriter) -> Result<Vec<(String, String, String, u8)>> {
    let output = writer.run(
        &["ls-files", "--stage", "-z"],
        crate::RunOptions {
            read_only: true,
            ..crate::RunOptions::default()
        },
    )?;
    Ok(output
        .stdout
        .split(|&b| b == 0)
        .filter_map(|record| {
            let record = std::str::from_utf8(record).ok()?;
            let (meta, path) = record.split_once('\t')?;
            let mut fields = meta.split(' ');
            let (mode, id, stage) = (fields.next()?, fields.next()?, fields.next()?);
            Some((
                path.to_owned(),
                mode.to_owned(),
                id.to_owned(),
                stage.parse().ok()?,
            ))
        })
        .collect())
}

/// Writes the recorded index entries of every path in `index` back: the
/// path is removed first, then its stage-0 entry or its unmerged entries
/// are added (`git update-index --index-info`).
pub(crate) fn write_index_snapshot(writer: &dyn GitWriter, index: &[IndexSnapshot]) -> Result<()> {
    if index.is_empty() {
        return Ok(());
    }
    let removals: Vec<(String, Option<(String, String)>)> =
        index.iter().map(|e| (e.path.clone(), None)).collect();
    writer.set_index_entries(&removals)?;
    let mut input = Vec::new();
    for entry in index {
        if let Some((mode, id)) = &entry.entry {
            input.extend_from_slice(format!("{mode} {id}\t{}", entry.path).as_bytes());
            input.push(0);
        }
        for (mode, id, stage) in &entry.stages {
            input.extend_from_slice(format!("{mode} {id} {stage}\t{}", entry.path).as_bytes());
            input.push(0);
        }
    }
    if input.is_empty() {
        return Ok(());
    }
    writer
        .run(
            &["update-index", "-z", "--index-info"],
            crate::RunOptions {
                stdin: Some(&input),
                read_only: false,
            },
        )
        .map(drop)
}

/// Records the index before a reset so undo can restore it (M9-04):
/// `git write-tree` refuses unmerged entries, so they are recorded entry by
/// entry and taken out for the moment; the rest is written as a tree, then
/// the unmerged entries go back (the index is unchanged afterwards).
/// Restore with [`restore_index`].
pub fn record_index(writer: &dyn GitWriter) -> Result<(String, Vec<IndexSnapshot>)> {
    let paths = crate::unmerged_paths(writer)?;
    let unmerged = index_snapshot(writer, &paths)?;
    if !paths.is_empty() {
        let mut input = Vec::new();
        for path in &paths {
            input.extend_from_slice(path.as_bytes());
            input.push(0);
        }
        writer.run(
            &["update-index", "--force-remove", "-z", "--stdin"],
            crate::RunOptions {
                stdin: Some(&input),
                read_only: false,
            },
        )?;
    }
    let tree = writer.run(&["write-tree"], crate::RunOptions::default());
    write_index_snapshot(writer, &unmerged)?;
    Ok((tree?.stdout_text().trim().to_owned(), unmerged))
}

/// Puts back an index recorded by [`record_index`].
pub fn restore_index(writer: &dyn GitWriter, tree: &str, unmerged: &[IndexSnapshot]) -> Result<()> {
    writer.run(&["read-tree", tree], crate::RunOptions::default())?;
    write_index_snapshot(writer, unmerged)
}

/// Discards conflicted `paths` completely (M9-03): back to HEAD in index and
/// working tree; a path that is not in HEAD leaves index and working tree.
fn discard_conflicted(writer: &dyn GitWriter, root: &Path, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let in_head: Vec<String> = if writer.has_head()? {
        let mut args = vec!["ls-tree", "-r", "--name-only", "-z", "HEAD", "--"];
        args.extend(paths.iter().map(String::as_str));
        let output = writer.run(
            &args,
            crate::RunOptions {
                read_only: true,
                ..crate::RunOptions::default()
            },
        )?;
        output
            .stdout
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect()
    } else {
        Vec::new()
    };
    let (head, new): (Vec<String>, Vec<String>) =
        paths.iter().cloned().partition(|p| in_head.contains(p));
    crate::cli_backend::with_pathspecs(
        writer,
        &["restore", "--source=HEAD", "--staged", "--worktree"],
        &head,
    )?;
    crate::cli_backend::with_pathspecs(writer, &["rm", "--cached", "--quiet", "--force"], &new)?;
    for path in &new {
        remove_file(root, path)?;
    }
    Ok(())
}

/// Discards everything in `tree`: staged changes go back to HEAD in index
/// and working tree, then unstaged changes and untracked files are discarded
/// like [`discard_all`].
pub fn discard_everything(
    writer: &dyn GitWriter,
    root: &Path,
    tree: &WorkingTree,
) -> Result<DiscardSnapshot> {
    let mut staged: Vec<String> = Vec::new();
    for file in &tree.staged {
        staged.extend(file.old_path.iter().cloned());
        staged.push(file.path.clone());
    }
    staged.sort();
    staged.dedup();
    let (conflicted, unstaged): (Vec<FileChange>, Vec<FileChange>) = tree
        .unstaged
        .iter()
        .filter(|f| staged.binary_search(&f.path).is_err())
        .cloned()
        .partition(|f| f.kind == ChangeKind::Conflicted);
    let conflicted: Vec<String> = conflicted.into_iter().map(|f| f.path).collect();
    let mut all: Vec<String> = staged.clone();
    all.extend(unstaged.iter().map(|f| f.path.clone()));
    all.extend(conflicted.iter().cloned());
    all.sort();
    all.dedup();
    let mut in_index = staged.clone();
    in_index.extend(conflicted.iter().cloned());

    let mut snapshot = snapshot(writer, root, &all)?;
    snapshot.index = index_snapshot(writer, &in_index)?;
    discard_conflicted(writer, root, &conflicted)?;
    if writer.has_head()? {
        writer.restore_from_head(&staged)?;
    } else {
        // Before the first commit everything staged is new.
        writer.unstage(&staged)?;
        for path in &staged {
            remove_file(root, path)?;
        }
    }
    let (untracked, tracked): (Vec<&FileChange>, Vec<&FileChange>) = unstaged
        .iter()
        .partition(|f| f.kind == ChangeKind::Untracked);
    let tracked: Vec<String> = tracked.iter().map(|f| f.path.clone()).collect();
    writer.restore_worktree(&tracked)?;
    for file in untracked {
        remove_file(root, &file.path)?;
    }
    Ok(snapshot)
}

/// Discards the working tree changes of `files`: tracked files go back to
/// their index content, untracked files are deleted, conflicted files go
/// back to HEAD in index and working tree (both sides are lost, M9-03).
pub fn discard_files(
    writer: &dyn GitWriter,
    root: &Path,
    files: &[FileChange],
) -> Result<DiscardSnapshot> {
    let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let mut snapshot = snapshot(writer, root, &paths)?;
    let (conflicted, files): (Vec<&FileChange>, Vec<&FileChange>) =
        files.iter().partition(|f| f.kind == ChangeKind::Conflicted);
    let conflicted: Vec<String> = conflicted.iter().map(|f| f.path.clone()).collect();
    snapshot.index = index_snapshot(writer, &conflicted)?;
    discard_conflicted(writer, root, &conflicted)?;
    let (untracked, tracked): (Vec<&FileChange>, Vec<&FileChange>) = files
        .into_iter()
        .partition(|f| f.kind == ChangeKind::Untracked);
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
pub(crate) fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
pub(crate) fn is_executable(_path: &Path) -> bool {
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
            encoding: None,
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
