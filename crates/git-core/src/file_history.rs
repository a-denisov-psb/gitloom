//! File history through the Git CLI (ADR-024): `git log --follow` so
//! renames, `.mailmap` and Git's own rules apply. Cancellable: the process
//! is stopped when the token is set.

use crate::{CancelToken, GitWriter, Result};

/// One commit that changed the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistoryEntry {
    pub id: String,
    pub author: String,
    pub email: String,
    /// Author time, Unix seconds.
    pub time: i64,
    pub summary: String,
    /// Path of the file in this commit.
    pub path: String,
    /// Path before a rename or copy in this commit.
    pub old_path: Option<String>,
    /// Git's status letter: `A`, `M`, `D`, `R`, `C`, `T` (empty for a
    /// commit without diff output, e.g. a merge).
    pub status: String,
}

impl FileHistoryEntry {
    pub fn renamed(&self) -> bool {
        self.old_path.is_some()
    }
}

const RECORD: u8 = 0x1e;
const FIELD: u8 = 0x1f;

/// Commits that changed `path`, newest first, following renames, starting
/// at revision `start` (e.g. `HEAD`).
pub fn file_history(
    git: &dyn GitWriter,
    path: &str,
    start: &str,
    cancel: &CancelToken,
) -> Result<Vec<FileHistoryEntry>> {
    let output = git.run_network(
        &[
            "--no-optional-locks",
            "log",
            "--follow",
            "-z",
            "--name-status",
            "--format=%x1e%H%x1f%aN%x1f%aE%x1f%at%x1f%s",
            start,
            "--",
            path,
        ],
        cancel,
        &mut |_| {},
    )?;
    Ok(parse_log(&output.stdout))
}

/// Files tracked in revision `rev` (`git ls-tree`), for the file list of the
/// palette.
pub fn tracked_files(git: &dyn GitWriter, rev: &str, cancel: &CancelToken) -> Result<Vec<String>> {
    let output = git.run_network(
        &[
            "--no-optional-locks",
            "ls-tree",
            "-r",
            "-z",
            "--name-only",
            rev,
        ],
        cancel,
        &mut |_| {},
    )?;
    Ok(output
        .stdout
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect())
}

/// Parses `log -z --name-status` output with the record format of
/// [`file_history`].
pub fn parse_log(output: &[u8]) -> Vec<FileHistoryEntry> {
    output
        .split(|&b| b == RECORD)
        .filter(|record| !record.is_empty())
        .filter_map(parse_record)
        .collect()
}

fn parse_record(record: &[u8]) -> Option<FileHistoryEntry> {
    // Header up to the first NUL, then `\n` and NUL-separated name-status.
    let header_end = record.iter().position(|&b| b == 0).unwrap_or(record.len());
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let mut fields = record[..header_end].split(|&b| b == FIELD);
    let id = text(fields.next()?);
    if id.is_empty() {
        return None;
    }
    let author = text(fields.next().unwrap_or_default());
    let email = text(fields.next().unwrap_or_default());
    let time = text(fields.next().unwrap_or_default()).parse().unwrap_or(0);
    let summary = text(fields.next().unwrap_or_default());

    let rest = record.get(header_end + 1..).unwrap_or_default();
    let mut tokens = rest
        .split(|&b| b == 0)
        .map(|t| t.strip_prefix(b"\n").unwrap_or(t))
        .filter(|t| !t.is_empty());
    let status = tokens.next().map(text).unwrap_or_default();
    let first = tokens.next().map(text);
    let (path, old_path) = if status.starts_with('R') || status.starts_with('C') {
        (tokens.next().map(text).unwrap_or_default(), first)
    } else {
        (first.unwrap_or_default(), None)
    };
    Some(FileHistoryEntry {
        id,
        author,
        email,
        time,
        summary,
        path,
        old_path,
        status: status.chars().next().map(String::from).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, summary: &str, status: &str, paths: &[&str]) -> Vec<u8> {
        let mut out = format!("\x1e{id}\x1fAda\x1fada@example.com\x1f1700000000\x1f{summary}\0\n")
            .into_bytes();
        out.extend_from_slice(status.as_bytes());
        out.push(0);
        for path in paths {
            out.extend_from_slice(path.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn parses_changes_and_renames() {
        let mut output = record("c3", "edit", "M", &["new name ü.txt"]);
        output.extend(record(
            "c2",
            "rename",
            "R087",
            &["old.txt", "new name ü.txt"],
        ));
        output.extend(record("c1", "add", "A", &["old.txt"]));
        let entries = parse_log(&output);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].path, "new name ü.txt");
        assert_eq!(entries[0].status, "M");
        assert!(!entries[0].renamed());
        assert_eq!(entries[1].old_path.as_deref(), Some("old.txt"));
        assert_eq!(entries[1].path, "new name ü.txt");
        assert_eq!(entries[1].status, "R");
        assert_eq!(entries[2].path, "old.txt");
        assert_eq!(entries[2].author, "Ada");
        assert_eq!(entries[2].email, "ada@example.com");
        assert_eq!(entries[2].time, 1_700_000_000);
        assert_eq!(entries[2].summary, "add");
    }

    #[test]
    fn commit_without_name_status_and_empty_output() {
        let output = b"\x1eabc\x1fA\x1fa@x\x1f5\x1fMerge branch 'x'\0\n".to_vec();
        let entries = parse_log(&output);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, "");
        assert_eq!(entries[0].path, "");
        assert!(parse_log(b"").is_empty());
    }

    #[test]
    fn copies_keep_the_source_as_old_path() {
        let entries = parse_log(&record("c", "copy", "C100", &["a.txt", "b.txt"]));
        assert_eq!(entries[0].old_path.as_deref(), Some("a.txt"));
        assert_eq!(entries[0].path, "b.txt");
    }
}
