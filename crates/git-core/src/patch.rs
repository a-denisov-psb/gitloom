//! Builds patches from a [`FileDiff`] for staging or unstaging whole hunks
//! or single lines with `git apply --cached`.

use std::fmt::Write as _;

use crate::{DiffLine, DiffLineKind, FileDiff};

/// What the patch is applied for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchDirection {
    /// Unstaged diff (index → worktree), applied forward to the index.
    Stage,
    /// Staged diff (HEAD → index), applied in reverse to the index.
    Unstage,
}

/// Builds a patch containing the changed lines for which `selected(hunk, line)`
/// is true. Returns `None` if no changed line is selected.
///
/// Unselected changes are turned into context or dropped so that the patch
/// still applies to the index:
/// - staging: unselected `+` lines are dropped, unselected `-` lines become context;
/// - unstaging (applied with `--reverse`): unselected `+` lines become context,
///   unselected `-` lines are dropped.
pub fn build_patch(
    diff: &FileDiff,
    direction: PatchDirection,
    selected: impl Fn(usize, usize) -> bool,
) -> Option<String> {
    let path = diff.new_path.as_ref().or(diff.old_path.as_ref())?;
    let old_path = diff.old_path.as_ref().unwrap_or(path);

    let changed: Vec<(usize, usize)> = diff
        .hunks
        .iter()
        .enumerate()
        .flat_map(|(h, hunk)| {
            hunk.lines
                .iter()
                .enumerate()
                .filter(|(_, l)| l.kind != DiffLineKind::Context)
                .map(move |(l, _)| (h, l))
        })
        .collect();
    let picked = changed.iter().filter(|(h, l)| selected(*h, *l)).count();
    if picked == 0 {
        return None;
    }
    let everything = picked == changed.len();

    // A new or deleted file keeps that nature only when all of it is selected
    // (a partial new file is still created; a partial deletion is an edit).
    let new_file = diff.old_path.is_none();
    let deleted_file = diff.new_path.is_none() && everything;

    let mut out = String::new();
    let _ = writeln!(out, "diff --git a/{old_path} b/{path}");
    if new_file {
        out.push_str("new file mode 100644\n");
    } else if deleted_file {
        out.push_str("deleted file mode 100644\n");
    }
    if new_file {
        out.push_str("--- /dev/null\n");
    } else {
        let _ = writeln!(out, "--- a/{old_path}");
    }
    if deleted_file {
        out.push_str("+++ /dev/null\n");
    } else {
        let _ = writeln!(out, "+++ b/{path}");
    }

    // Offset of the side whose positions change through the selection.
    let mut shift: i64 = 0;
    for (h, hunk) in diff.hunks.iter().enumerate() {
        let mut lines: Vec<(char, &DiffLine)> = Vec::new();
        let mut any = false;
        for (l, line) in hunk.lines.iter().enumerate() {
            let pick = selected(h, l);
            let marker = match (line.kind, pick, direction) {
                (DiffLineKind::Context, _, _) => Some(' '),
                (DiffLineKind::Added, true, _) => Some('+'),
                (DiffLineKind::Removed, true, _) => Some('-'),
                (DiffLineKind::Added, false, PatchDirection::Stage) => None,
                (DiffLineKind::Removed, false, PatchDirection::Stage) => Some(' '),
                (DiffLineKind::Added, false, PatchDirection::Unstage) => Some(' '),
                (DiffLineKind::Removed, false, PatchDirection::Unstage) => None,
            };
            any |= pick && line.kind != DiffLineKind::Context;
            if let Some(marker) = marker {
                lines.push((marker, line));
            }
        }
        let old_count = lines.iter().filter(|(m, _)| *m != '+').count() as i64;
        let new_count = lines.iter().filter(|(m, _)| *m != '-').count() as i64;
        if !any {
            // Hunk not part of the patch; its unselected changes still move later positions.
            match direction {
                PatchDirection::Stage => {}
                PatchDirection::Unstage => {
                    shift += i64::from(hunk.new_count) - i64::from(hunk.old_count);
                }
            }
            continue;
        }

        // Staging keeps the old (index) side exact and derives the new side;
        // unstaging keeps the new (index) side exact and derives the old side.
        let (old_start, new_start) = match direction {
            PatchDirection::Stage => {
                let old = i64::from(hunk.old_start);
                (old, start_for(old, old_count, new_count, shift))
            }
            PatchDirection::Unstage => {
                let new = i64::from(hunk.new_start);
                (start_for(new, new_count, old_count, -shift), new)
            }
        };
        let _ = writeln!(
            out,
            "@@ -{} +{} @@",
            range(old_start, old_count),
            range(new_start, new_count)
        );
        for (marker, line) in lines {
            out.push(marker);
            out.push_str(&line.content);
            if line.missing_newline {
                out.push('\n');
                out.push_str("\\ No newline at end of file\n");
            } else if line.crlf {
                out.push_str("\r\n");
            } else {
                out.push('\n');
            }
        }
        shift += match direction {
            PatchDirection::Stage => new_count - old_count,
            PatchDirection::Unstage => old_count - new_count,
        };
    }
    Some(out)
}

/// Start line on the derived side: shifted by the changes of earlier hunks.
/// An empty range starts before its position (Git convention).
fn start_for(exact_start: i64, exact_count: i64, derived_count: i64, shift: i64) -> i64 {
    // `exact_start` is 0 for an empty exact range (e.g. new file); turn it into the
    // line before which the hunk applies, shift it, and convert back.
    let before = if exact_count == 0 {
        exact_start + 1
    } else {
        exact_start
    };
    let start = before + shift;
    if derived_count == 0 {
        (start - 1).max(0)
    } else {
        start.max(1)
    }
}

fn range(start: i64, count: i64) -> String {
    format!("{start},{count}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::text_diff;

    fn file_diff(old: &str, new: &str) -> FileDiff {
        let text = text_diff(old.as_bytes(), new.as_bytes());
        FileDiff {
            old_path: (!old.is_empty() || !new.is_empty()).then(|| "f.txt".to_owned()),
            new_path: Some("f.txt".into()),
            binary: false,
            too_large: false,
            encoding: None,
            hunks: text.hunks,
        }
    }

    #[test]
    fn nothing_selected_gives_no_patch() {
        let diff = file_diff("a\n", "b\n");
        assert!(build_patch(&diff, PatchDirection::Stage, |_, _| false).is_none());
    }

    #[test]
    fn full_hunk_patch() {
        let diff = file_diff("a\nb\nc\n", "a\nB\nc\n");
        let patch = build_patch(&diff, PatchDirection::Stage, |_, _| true).unwrap();
        assert_eq!(
            patch,
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,3 +1,3 @@\n a\n-b\n+B\nc\n"
                .replace("\nc\n", "\n c\n")
        );
    }

    #[test]
    fn staging_only_the_addition_keeps_the_removal_as_context() {
        let diff = file_diff("a\nb\nc\n", "a\nB\nc\n");
        // Lines: " a", "-b", "+B", " c" → select only "+B".
        let patch = build_patch(&diff, PatchDirection::Stage, |_, l| l == 2).unwrap();
        assert!(
            patch.ends_with("@@ -1,3 +1,4 @@\n a\n b\n+B\n c\n"),
            "{patch}"
        );
    }

    #[test]
    fn unstaging_only_the_removal_keeps_the_addition_as_context() {
        let diff = file_diff("a\nb\nc\n", "a\nB\nc\n");
        let patch = build_patch(&diff, PatchDirection::Unstage, |_, l| l == 1).unwrap();
        assert!(
            patch.ends_with("@@ -1,4 +1,3 @@\n a\n-b\n B\n c\n"),
            "{patch}"
        );
    }

    #[test]
    fn crlf_lines_keep_their_ending() {
        let diff = file_diff("a\r\nb\r\n", "a\r\nB\r\n");
        let patch = build_patch(&diff, PatchDirection::Stage, |_, _| true).unwrap();
        assert!(patch.contains("-b\r\n+B\r\n"), "{patch:?}");
    }

    #[test]
    fn missing_newline_marker() {
        let diff = file_diff("a\nb", "a\nb\n");
        let patch = build_patch(&diff, PatchDirection::Stage, |_, _| true).unwrap();
        assert!(
            patch.contains("-b\n\\ No newline at end of file\n+b\n"),
            "{patch}"
        );
    }
}
