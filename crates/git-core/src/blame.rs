//! Blame through the Git CLI (ADR-024): `git blame --porcelain`, so
//! `.mailmap` and `blame.ignoreRevsFile` apply exactly as in Git.

use crate::{CancelToken, GitWriter, Result};

/// Commit id Git uses for lines that are not committed yet.
pub const UNCOMMITTED: &str = "0000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameCommit {
    pub id: String,
    pub author: String,
    pub email: String,
    /// Author time, Unix seconds.
    pub time: i64,
    pub summary: String,
    /// The commit is a root or the boundary of the blamed range.
    pub boundary: bool,
}

impl BlameCommit {
    pub fn is_uncommitted(&self) -> bool {
        self.id == UNCOMMITTED
    }
}

/// Consecutive lines last changed by the same commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlameGroup {
    /// Index into [`Blame::commits`].
    pub commit: usize,
    /// First line (0-based) and number of lines.
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blame {
    pub commits: Vec<BlameCommit>,
    /// Content of each line (without line ending) and its commit index.
    pub lines: Vec<(usize, String)>,
    pub groups: Vec<BlameGroup>,
    /// The file contains NUL bytes; lines are not meaningful.
    pub binary: bool,
}

/// Blames `path` at revision `rev`, or the working copy (uncommitted lines
/// get [`UNCOMMITTED`]) when `rev` is `None`.
pub fn blame(
    git: &dyn GitWriter,
    path: &str,
    rev: Option<&str>,
    cancel: &CancelToken,
) -> Result<Blame> {
    let mut args = vec!["--no-optional-locks", "blame", "--porcelain"];
    args.extend(rev);
    args.extend(["--", path]);
    let output = git.run_network(&args, cancel, &mut |_| {})?;
    Ok(parse_porcelain(&output.stdout))
}

/// Parses `git blame --porcelain` output.
pub fn parse_porcelain(output: &[u8]) -> Blame {
    let mut blame = Blame::default();
    let mut index_of: std::collections::HashMap<String, usize> = Default::default();
    let mut current: Option<usize> = None;
    for raw in output.split(|&b| b == b'\n') {
        if let Some(content) = raw.strip_prefix(b"\t") {
            let Some(commit) = current else {
                continue;
            };
            if content.contains(&0) {
                blame.binary = true;
            }
            let content = content.strip_suffix(b"\r").unwrap_or(content);
            blame
                .lines
                .push((commit, String::from_utf8_lossy(content).into_owned()));
            continue;
        }
        let line = String::from_utf8_lossy(raw);
        let (key, value) = line.split_once(' ').unwrap_or((&line, ""));
        // Header of a line: "<40 hex> <orig> <final> [<count>]".
        if key.len() == 40 && key.bytes().all(|b| b.is_ascii_hexdigit()) {
            let next = blame.commits.len();
            let index = *index_of.entry(key.to_owned()).or_insert(next);
            if index == next {
                blame.commits.push(BlameCommit {
                    id: key.to_owned(),
                    author: String::new(),
                    email: String::new(),
                    time: 0,
                    summary: String::new(),
                    boundary: false,
                });
            }
            current = Some(index);
            continue;
        }
        let Some(commit) = current.and_then(|i| blame.commits.get_mut(i)) else {
            continue;
        };
        match key {
            "author" => commit.author = value.to_owned(),
            "author-mail" => {
                commit.email = value
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .to_owned()
            }
            "author-time" => commit.time = value.parse().unwrap_or(0),
            "summary" => commit.summary = value.to_owned(),
            "boundary" => commit.boundary = true,
            _ => {}
        }
    }
    blame.groups = group(&blame.lines);
    blame
}

/// Runs of consecutive lines with the same commit.
fn group(lines: &[(usize, String)]) -> Vec<BlameGroup> {
    let mut groups: Vec<BlameGroup> = Vec::new();
    for (index, (commit, _)) in lines.iter().enumerate() {
        match groups.last_mut() {
            Some(last) if last.commit == *commit => last.len += 1,
            _ => groups.push(BlameGroup {
                commit: *commit,
                start: index,
                len: 1,
            }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn header(id: &str, line: usize, first: bool, summary: &str, extra: &str) -> String {
        let mut out = format!("{id} {line} {line} 1\n");
        if first {
            out.push_str(&format!(
                "author Ada\nauthor-mail <ada@example.com>\nauthor-time 1700000000\n\
                 author-tz +0100\ncommitter Ada\ncommitter-mail <ada@example.com>\n\
                 committer-time 1700000000\ncommitter-tz +0100\nsummary {summary}\n{extra}\
                 filename f.txt\n"
            ));
        }
        out
    }

    #[test]
    fn repeated_commits_are_listed_once_and_grouped() {
        let mut out = header(A, 1, true, "first", "boundary\n");
        out.push_str("\tone\n");
        out.push_str(&header(A, 2, false, "", ""));
        out.push_str("\ttwo\n");
        out.push_str(&header(
            B,
            3,
            true,
            "second",
            &format!("previous {A} f.txt\n"),
        ));
        out.push_str("\tthree\n");
        out.push_str(&header(A, 4, false, "", ""));
        out.push_str("\tfour\n");
        let blame = parse_porcelain(out.as_bytes());
        assert_eq!(blame.commits.len(), 2);
        assert_eq!(blame.commits[0].summary, "first");
        assert!(blame.commits[0].boundary, "boundary commit");
        assert!(!blame.commits[1].boundary);
        assert_eq!(blame.commits[1].email, "ada@example.com");
        assert_eq!(blame.commits[1].time, 1_700_000_000);
        assert_eq!(
            blame
                .lines
                .iter()
                .map(|(c, l)| (*c, l.as_str()))
                .collect::<Vec<_>>(),
            [(0, "one"), (0, "two"), (1, "three"), (0, "four")]
        );
        assert_eq!(
            blame.groups,
            [
                BlameGroup {
                    commit: 0,
                    start: 0,
                    len: 2
                },
                BlameGroup {
                    commit: 1,
                    start: 2,
                    len: 1
                },
                BlameGroup {
                    commit: 0,
                    start: 3,
                    len: 1
                },
            ]
        );
    }

    #[test]
    fn uncommitted_lines() {
        let mut out = header(UNCOMMITTED, 1, true, "Version of f.txt from f.txt", "");
        out.push_str("\tnew\n");
        let blame = parse_porcelain(out.as_bytes());
        assert!(blame.commits[0].is_uncommitted());
    }

    #[test]
    fn crlf_line_endings_and_tabs_in_content() {
        let mut out = header(A, 1, true, "s", "").into_bytes();
        out.extend_from_slice(b"\tx\r\n");
        out.extend_from_slice(header(A, 2, false, "", "").as_bytes());
        out.extend_from_slice(b"\t\tindented\r\n");
        let blame = parse_porcelain(&out);
        assert_eq!(blame.lines[0].1, "x");
        assert_eq!(blame.lines[1].1, "\tindented");
        assert!(!blame.binary);
    }

    #[test]
    fn binary_content_is_flagged_and_empty_output_is_empty() {
        let mut out = header(A, 1, true, "s", "").into_bytes();
        out.extend_from_slice(b"\tab\0cd\n");
        assert!(parse_porcelain(&out).binary);
        assert_eq!(parse_porcelain(b""), Blame::default());
    }
}
