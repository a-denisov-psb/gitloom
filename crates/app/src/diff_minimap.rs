//! Minimap of the Inline and Split diff (M15-09): one mark per run of
//! changed rows at its position among the visual lines (rows after word
//! wrap). Pure; drawn by `ui::diff`.

use git_core::{DiffLineKind, FileDiff};

use crate::diff_wrap::WrapLayout;
use crate::repo_view::DiffRow;

/// Width of the minimap strip.
pub const WIDTH: f32 = 14.0;
/// Smallest mark height in pixels.
pub const MIN_MARK: f32 = 2.0;

/// Which part of the strip a mark covers: all of it (Inline), or the left
/// (removed) or right (added) half of a Split diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    Full,
    Left,
    Right,
}

/// A run of changed visual lines of one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// First visual line.
    pub start: u32,
    /// Visual lines covered.
    pub lines: u32,
    /// `true` for added, `false` for removed lines.
    pub added: bool,
    pub half: Half,
}

/// Marks of a diff layout and the number of visual lines they refer to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Marks {
    pub marks: Vec<Mark>,
    pub total: u32,
}

/// The marks of `rows` of `file`, with `wrap` when word wrap is on.
pub fn marks(rows: &[DiffRow], file: &FileDiff, wrap: Option<&WrapLayout>) -> Marks {
    let start = |row: usize| wrap.map_or(row as u32, |w| w.start(row));
    let lines = |row: usize| wrap.map_or(1, |w| w.lines(row));
    let kind = |h: u32, l: u32| {
        file.hunks
            .get(h as usize)
            .and_then(|hunk| hunk.lines.get(l as usize))
            .map(|line| line.kind)
    };
    let mut marks: Vec<Mark> = Vec::new();
    let mut push = |mark: Mark| {
        // Extend the last mark of the same kind and half when it touches.
        if let Some(last) = marks
            .iter_mut()
            .rev()
            .take(2)
            .find(|m| m.added == mark.added && m.half == mark.half)
        {
            if last.start + last.lines == mark.start {
                last.lines += mark.lines;
                return;
            }
        }
        marks.push(mark);
    };
    for (index, row) in rows.iter().enumerate() {
        let (start, lines) = (start(index), lines(index));
        match *row {
            DiffRow::Line(h, l) => {
                let added = match kind(h, l) {
                    Some(DiffLineKind::Added) => true,
                    Some(DiffLineKind::Removed) => false,
                    _ => continue,
                };
                push(Mark {
                    start,
                    lines,
                    added,
                    half: Half::Full,
                });
            }
            DiffRow::Split(h, left, right) => {
                if left.is_some_and(|l| kind(h, l) == Some(DiffLineKind::Removed)) {
                    push(Mark {
                        start,
                        lines,
                        added: false,
                        half: Half::Left,
                    });
                }
                if right.is_some_and(|r| kind(h, r) == Some(DiffLineKind::Added)) {
                    push(Mark {
                        start,
                        lines,
                        added: true,
                        half: Half::Right,
                    });
                }
            }
            DiffRow::Header(_) | DiffRow::Block(_) => {}
        }
    }
    let total = wrap.map_or(rows.len() as u32, WrapLayout::total);
    Marks { marks, total }
}

/// Top and height in pixels of `mark` on a strip `height` high showing
/// `total` visual lines (at least [`MIN_MARK`] high).
pub fn mark_rect(mark: &Mark, total: u32, height: f32) -> (f32, f32) {
    let total = total.max(1) as f32;
    let top = mark.start as f32 / total * height;
    let size = (mark.lines as f32 / total * height).max(MIN_MARK);
    (top.min(height - MIN_MARK).max(0.0), size)
}

/// Scroll offset that centers the view (`viewport` pixels high) on the
/// fraction `at` (0 … 1) of content `content` pixels high.
pub fn centered_offset(at: f32, content: f32, viewport: f32) -> f32 {
    let max = (content - viewport).max(0.0);
    (at.clamp(0.0, 1.0) * content - viewport / 2.0).clamp(0.0, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::{DiffHunk, DiffLine};

    fn line(kind: DiffLineKind) -> DiffLine {
        DiffLine {
            kind,
            old_line: None,
            new_line: None,
            content: "x".into(),
            missing_newline: false,
            crlf: false,
        }
    }

    fn file(kinds: &[DiffLineKind]) -> FileDiff {
        FileDiff {
            old_path: Some("a".into()),
            new_path: Some("a".into()),
            binary: false,
            too_large: false,
            encoding: None,
            hunks: vec![DiffHunk {
                header: "@@".into(),
                old_start: 1,
                old_count: 0,
                new_start: 1,
                new_count: 0,
                lines: kinds.iter().map(|k| line(*k)).collect(),
            }],
        }
    }

    use DiffLineKind::{Added as A, Context as C, Removed as R};

    #[test]
    fn inline_marks_runs_of_changed_rows() {
        let file = file(&[C, R, R, A, C, C, A]);
        let mut rows = vec![DiffRow::Block(0)];
        rows.extend((0..7).map(|l| DiffRow::Line(0, l)));
        let marks = marks(&rows, &file, None);
        assert_eq!(marks.total, 8);
        assert_eq!(
            marks.marks,
            [
                Mark {
                    start: 2,
                    lines: 2,
                    added: false,
                    half: Half::Full
                },
                Mark {
                    start: 4,
                    lines: 1,
                    added: true,
                    half: Half::Full
                },
                Mark {
                    start: 7,
                    lines: 1,
                    added: true,
                    half: Half::Full
                },
            ]
        );
    }

    #[test]
    fn word_wrap_counts_visual_lines() {
        use crate::diff_wrap::RowChars;
        let file = file(&[C, R, A]);
        let rows: Vec<DiffRow> = (0..3).map(|l| DiffRow::Line(0, l)).collect();
        // The context line wraps into 3 visual lines, the removed one into 2.
        let chars = [
            RowChars::single(25),
            RowChars::single(15),
            RowChars::single(5),
        ];
        let wrap = WrapLayout::build(&chars, 10);
        let marks = marks(&rows, &file, Some(&wrap));
        assert_eq!(marks.total, 6);
        assert_eq!(
            marks.marks,
            [
                Mark {
                    start: 3,
                    lines: 2,
                    added: false,
                    half: Half::Full
                },
                Mark {
                    start: 5,
                    lines: 1,
                    added: true,
                    half: Half::Full
                },
            ]
        );
    }

    #[test]
    fn split_puts_removed_left_and_added_right() {
        // Hunk lines: 0 context, 1 removed, 2 removed, 3 added.
        let file = file(&[C, R, R, A]);
        let rows = [
            DiffRow::Split(0, Some(0), Some(0)),
            DiffRow::Split(0, Some(1), Some(3)),
            DiffRow::Split(0, Some(2), None),
        ];
        let marks = marks(&rows, &file, None);
        assert_eq!(
            marks.marks,
            [
                Mark {
                    start: 1,
                    lines: 2,
                    added: false,
                    half: Half::Left
                },
                Mark {
                    start: 1,
                    lines: 1,
                    added: true,
                    half: Half::Right
                },
            ]
        );
    }

    #[test]
    fn rects_and_offsets() {
        let mark = Mark {
            start: 50,
            lines: 1,
            added: true,
            half: Half::Full,
        };
        // A single line of 20,000 still gets 2 px.
        assert_eq!(mark_rect(&mark, 20_000, 400.0), (1.0, MIN_MARK));
        let mark = Mark {
            start: 0,
            lines: 100,
            added: true,
            half: Half::Full,
        };
        assert_eq!(mark_rect(&mark, 200, 400.0), (0.0, 200.0));
        // The end stays inside the strip.
        let mark = Mark {
            start: 199,
            lines: 1,
            added: false,
            half: Half::Full,
        };
        assert_eq!(mark_rect(&mark, 200, 400.0).0, 398.0);
        assert_eq!(centered_offset(0.5, 1000.0, 200.0), 400.0);
        assert_eq!(centered_offset(0.0, 1000.0, 200.0), 0.0);
        assert_eq!(centered_offset(1.0, 1000.0, 200.0), 800.0);
        assert_eq!(centered_offset(0.5, 100.0, 200.0), 0.0);
    }
}
