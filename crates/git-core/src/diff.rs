//! Line diff of two blobs, grouped into unified hunks.

use gix::diff::blob::{sources, Algorithm, Diff, InternedInput};

use crate::{DiffHunk, DiffLine, DiffLineKind};

/// Files above this size (either side) are not diffed.
pub const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;
/// Unchanged lines shown around each change.
const CONTEXT: u32 = 3;
/// Bytes inspected for NUL when detecting binary content (same as Git).
const BINARY_PROBE: usize = 8000;

/// Outcome of diffing two blobs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TextDiff {
    pub binary: bool,
    pub too_large: bool,
    pub hunks: Vec<DiffHunk>,
}

pub fn is_binary(data: &[u8]) -> bool {
    data[..data.len().min(BINARY_PROBE)].contains(&0)
}

/// Diffs `old` against `new` line by line with Git's default context of 3 lines.
pub fn text_diff(old: &[u8], new: &[u8]) -> TextDiff {
    if old.len() > MAX_DIFF_BYTES || new.len() > MAX_DIFF_BYTES {
        return TextDiff {
            too_large: true,
            ..TextDiff::default()
        };
    }
    if is_binary(old) || is_binary(new) {
        return TextDiff {
            binary: true,
            ..TextDiff::default()
        };
    }

    let input = InternedInput::new(sources::byte_lines(old), sources::byte_lines(new));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let changes: Vec<_> = diff.hunks().collect();

    let old_lines: Vec<&[u8]> = input.before.iter().map(|t| input.interner[*t]).collect();
    let new_lines: Vec<&[u8]> = input.after.iter().map(|t| input.interner[*t]).collect();
    let (old_len, new_len) = (old_lines.len() as u32, new_lines.len() as u32);

    let mut hunks = Vec::new();
    let mut start = 0;
    while start < changes.len() {
        // Merge changes whose context would overlap or touch.
        let mut end = start + 1;
        while end < changes.len()
            && changes[end].before.start - changes[end - 1].before.end <= 2 * CONTEXT
        {
            end += 1;
        }
        let group = &changes[start..end];
        let (first, last) = (&group[0], &group[group.len() - 1]);

        let old_start = first.before.start.saturating_sub(CONTEXT);
        let new_start = first.after.start - (first.before.start - old_start);
        let old_end = (last.before.end + CONTEXT).min(old_len);
        let new_end = last.after.end + (old_end - last.before.end);

        let mut lines = Vec::new();
        let (mut o, mut n) = (old_start, new_start);
        let line = |kind, old: Option<u32>, new: Option<u32>, data: &[u8], last: bool| DiffLine {
            kind,
            old_line: old.map(|i| i + 1),
            new_line: new.map(|i| i + 1),
            content: display(data),
            missing_newline: last && !data.ends_with(b"\n"),
        };
        for change in group {
            while o < change.before.start {
                let data = old_lines[o as usize];
                lines.push(line(
                    DiffLineKind::Context,
                    Some(o),
                    Some(n),
                    data,
                    o + 1 == old_len,
                ));
                o += 1;
                n += 1;
            }
            for i in change.before.clone() {
                let data = old_lines[i as usize];
                lines.push(line(
                    DiffLineKind::Removed,
                    Some(i),
                    None,
                    data,
                    i + 1 == old_len,
                ));
            }
            for i in change.after.clone() {
                let data = new_lines[i as usize];
                lines.push(line(
                    DiffLineKind::Added,
                    None,
                    Some(i),
                    data,
                    i + 1 == new_len,
                ));
            }
            o = change.before.end;
            n = change.after.end;
        }
        while o < old_end {
            let data = old_lines[o as usize];
            lines.push(line(
                DiffLineKind::Context,
                Some(o),
                Some(n),
                data,
                o + 1 == old_len,
            ));
            o += 1;
            n += 1;
        }

        let (old_count, new_count) = (old_end - old_start, new_end - new_start);
        hunks.push(DiffHunk {
            header: format!(
                "@@ -{} +{} @@",
                range(old_start, old_count),
                range(new_start, new_count)
            ),
            old_start: old_start + u32::from(old_count > 0),
            old_count,
            new_start: new_start + u32::from(new_count > 0),
            new_count,
            lines,
        });
        start = end;
    }

    TextDiff {
        hunks,
        ..TextDiff::default()
    }
}

/// Unified diff range: 1-based start, count omitted when 1; empty ranges start before the line.
fn range(start: u32, count: u32) -> String {
    match count {
        0 => format!("{start},0"),
        1 => format!("{}", start + 1),
        _ => format!("{},{count}", start + 1),
    }
}

/// Line text without its terminator.
fn display(line: &[u8]) -> String {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    String::from_utf8_lossy(line).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders hunks like `git diff` (without file headers).
    fn unified(diff: &TextDiff) -> String {
        let mut out = String::new();
        for hunk in &diff.hunks {
            out.push_str(&hunk.header);
            out.push('\n');
            for line in &hunk.lines {
                let marker = match line.kind {
                    DiffLineKind::Context => ' ',
                    DiffLineKind::Added => '+',
                    DiffLineKind::Removed => '-',
                };
                out.push(marker);
                out.push_str(&line.content);
                out.push('\n');
                if line.missing_newline {
                    out.push_str("\\ No newline at end of file\n");
                }
            }
        }
        out
    }

    fn numbered(range: std::ops::Range<u32>) -> String {
        range.map(|i| format!("line {i}\n")).collect()
    }

    #[test]
    fn identical_input_has_no_hunks() {
        assert_eq!(text_diff(b"a\nb\n", b"a\nb\n"), TextDiff::default());
    }

    #[test]
    fn single_change_with_context() {
        let old = numbered(1..11);
        let new = old.replace("line 5\n", "line five\n");
        let diff = text_diff(old.as_bytes(), new.as_bytes());
        assert_eq!(
            unified(&diff),
            "@@ -2,7 +2,7 @@\n line 2\n line 3\n line 4\n-line 5\n+line five\n line 6\n line 7\n line 8\n"
        );
        let hunk = &diff.hunks[0];
        assert_eq!(
            (
                hunk.old_start,
                hunk.old_count,
                hunk.new_start,
                hunk.new_count
            ),
            (2, 7, 2, 7)
        );
        let removed = &hunk.lines[3];
        assert_eq!((removed.old_line, removed.new_line), (Some(5), None));
        let after = &hunk.lines[5];
        assert_eq!((after.old_line, after.new_line), (Some(6), Some(6)));
    }

    #[test]
    fn distant_changes_make_separate_hunks() {
        let old = numbered(1..21);
        let new = old
            .replace("line 2\n", "two\n")
            .replace("line 18\n", "eighteen\n");
        let diff = text_diff(old.as_bytes(), new.as_bytes());
        let headers: Vec<_> = diff.hunks.iter().map(|h| h.header.as_str()).collect();
        assert_eq!(headers, ["@@ -1,5 +1,5 @@", "@@ -15,6 +15,6 @@"]);
    }

    #[test]
    fn close_changes_share_a_hunk() {
        let old = numbered(1..21);
        let new = old
            .replace("line 5\n", "five\n")
            .replace("line 11\n", "eleven\n");
        let diff = text_diff(old.as_bytes(), new.as_bytes());
        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].header, "@@ -2,13 +2,13 @@");
    }

    #[test]
    fn added_file() {
        let diff = text_diff(b"", b"a\nb\n");
        assert_eq!(unified(&diff), "@@ -0,0 +1,2 @@\n+a\n+b\n");
        assert_eq!((diff.hunks[0].old_start, diff.hunks[0].new_start), (0, 1));
    }

    #[test]
    fn deleted_file() {
        let diff = text_diff(b"a\n", b"");
        assert_eq!(unified(&diff), "@@ -1 +0,0 @@\n-a\n");
    }

    #[test]
    fn empty_files() {
        assert!(text_diff(b"", b"").hunks.is_empty());
    }

    #[test]
    fn missing_trailing_newline() {
        let diff = text_diff(b"a\nb", b"a\nb\n");
        assert_eq!(
            unified(&diff),
            "@@ -1,2 +1,2 @@\n a\n-b\n\\ No newline at end of file\n+b\n"
        );
    }

    #[test]
    fn crlf_is_stripped_for_display() {
        let diff = text_diff(b"a\r\n", b"b\r\n");
        assert_eq!(unified(&diff), "@@ -1 +1 @@\n-a\n+b\n");
    }

    #[test]
    fn binary_content() {
        let diff = text_diff(b"text\n", b"bin\0ary");
        assert!(diff.binary && diff.hunks.is_empty());
    }

    #[test]
    fn too_large_content() {
        let big = vec![b'a'; MAX_DIFF_BYTES + 1];
        let diff = text_diff(b"", &big);
        assert!(diff.too_large && !diff.binary && diff.hunks.is_empty());
    }
}
