//! Content of a file as it is in a commit (M19-01, ADR-061), for the
//! read-only copies the external editor opens.

use crate::{Error, GitWriter, Result, RunOptions};

/// What a path is in a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitFile {
    /// The file's bytes as a checkout would write them (line endings and
    /// other filters applied).
    Content(Vec<u8>),
    /// The path does not exist in the commit.
    Missing,
    /// The path is a folder.
    Folder,
    /// The path is a submodule (a commit id, no content).
    Submodule,
}

fn read_only() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

/// Reads `path` (relative to the repository root, `/` separated) as it is
/// in commit `rev` (an id or any revision such as `abc123^1`). No size
/// limit. Runs Git processes: call it off the UI thread.
pub fn file_at_commit(git: &dyn GitWriter, rev: &str, path: &str) -> Result<CommitFile> {
    if path.is_empty() {
        return Ok(CommitFile::Missing);
    }
    // `ls-tree` reports the entry without needing its object (submodules);
    // literal pathspecs so `*` or `?` in a name match only themselves.
    let output = git.run_with_env(
        &["ls-tree", "-z", "--full-tree", rev, "--", path],
        &[("GIT_LITERAL_PATHSPECS", "1")],
        read_only(),
    )?;
    let Some((kind, id)) = parse_entry(&output.stdout, path) else {
        return Ok(CommitFile::Missing);
    };
    match kind.as_str() {
        "blob" => git.blob_for_worktree(path, &id).map(CommitFile::Content),
        "tree" => Ok(CommitFile::Folder),
        "commit" => Ok(CommitFile::Submodule),
        other => Err(Error::Backend(format!(
            "Unexpected object type {other} for {path}"
        ))),
    }
}

/// Full id of the commit `rev` names; `None` if there is none (e.g. the
/// parent of a root commit).
pub fn commit_id(git: &dyn GitWriter, rev: &str) -> Result<Option<String>> {
    let spec = format!("{rev}^{{commit}}");
    match git.run(&["rev-parse", "--verify", "--quiet", &spec], read_only()) {
        Ok(output) => Ok(Some(output.stdout_text().trim().to_owned())),
        Err(Error::Command { code: Some(1), .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Type and object id of the `ls-tree -z` entry named exactly `path`.
fn parse_entry(output: &[u8], path: &str) -> Option<(String, String)> {
    output
        .split(|b| *b == 0)
        .filter(|record| !record.is_empty())
        .find_map(|record| {
            let record = String::from_utf8_lossy(record);
            let (meta, name) = record.split_once('\t')?;
            if name != path {
                return None;
            }
            let mut parts = meta.split(' ');
            let _mode = parts.next()?;
            let kind = parts.next()?.to_owned();
            let id = parts.next()?.to_owned();
            Some((kind, id))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_matching_entry() {
        let output = b"100644 blob abc\ta.txt\x00040000 tree def\tdir\x00160000 commit 123\tsub\0";
        assert_eq!(
            parse_entry(output, "a.txt"),
            Some(("blob".into(), "abc".into()))
        );
        assert_eq!(
            parse_entry(output, "dir"),
            Some(("tree".into(), "def".into()))
        );
        assert_eq!(
            parse_entry(output, "sub"),
            Some(("commit".into(), "123".into()))
        );
        assert_eq!(parse_entry(output, "a"), None);
        assert_eq!(parse_entry(b"", "a.txt"), None);
    }

    #[test]
    fn names_with_tabs_and_spaces() {
        let output = b"100644 blob abc\tmy file\twith tab\0";
        assert_eq!(
            parse_entry(output, "my file\twith tab"),
            Some(("blob".into(), "abc".into()))
        );
    }
}
