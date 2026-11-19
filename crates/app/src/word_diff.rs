//! Changed characters within changed lines (word diff, fix after M15):
//! GitKraken highlights the parts of a changed line that differ from the
//! line it replaced. Pure: no iced types, computed once per loaded diff in
//! a background task and kept with the diff view.
//!
//! Pairing: within a change block (removed lines directly followed by
//! added lines) the i-th removed line is compared with the i-th added
//! line; extra lines of the longer side get no emphasis. Lines are cut
//! into tokens (words, whitespace runs, single other characters) and the
//! longest common token sequence is kept; adjacent changed tokens form one
//! range.

use std::collections::HashMap;
use std::ops::Range;

use git_core::{DiffLineKind, FileDiff};

/// Lines longer than this (in characters) get no emphasis.
pub const MAX_LINE_CHARS: usize = 1_000;

/// Least share of unchanged characters (of both lines together, without
/// whitespace, which nearly every code line shares as indentation) for an
/// emphasis: below it the lines have too little in common and every token
/// would light up, which is only noise.
pub const MIN_UNCHANGED: f64 = 0.4;

/// Largest table of the token comparison after the common start and end
/// are cut off; larger middles count as changed as a whole.
const MAX_CELLS: usize = 1_000_000;

/// Changed byte ranges of the paired lines of a diff, by line number:
/// removed lines by their old number, added lines by their new number.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Emphasis {
    removed: HashMap<u32, Vec<Range<usize>>>,
    added: HashMap<u32, Vec<Range<usize>>>,
}

impl Emphasis {
    /// Changed byte ranges of `line` (empty for unpaired and context lines).
    pub fn ranges(&self, line: &git_core::DiffLine) -> &[Range<usize>] {
        let found = match line.kind {
            DiffLineKind::Removed => line.old_line.and_then(|n| self.removed.get(&n)),
            DiffLineKind::Added => line.new_line.and_then(|n| self.added.get(&n)),
            DiffLineKind::Context => None,
        };
        found.map_or(&[], Vec::as_slice)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.added.is_empty()
    }
}

/// Word diff of every change block of `diff`.
pub fn of_diff(diff: &FileDiff) -> Emphasis {
    let mut emphasis = Emphasis::default();
    for hunk in &diff.hunks {
        let lines = &hunk.lines;
        let mut at = 0;
        while at < lines.len() {
            let removed_start = at;
            while at < lines.len() && lines[at].kind == DiffLineKind::Removed {
                at += 1;
            }
            let added_start = at;
            while at < lines.len() && lines[at].kind == DiffLineKind::Added {
                at += 1;
            }
            if removed_start == at {
                // A context line.
                at += 1;
                continue;
            }
            let removed = &lines[removed_start..added_start];
            let added = &lines[added_start..at];
            for (old, new) in removed.iter().zip(added) {
                let (Some(old_number), Some(new_number)) = (old.old_line, new.new_line) else {
                    continue;
                };
                if let Some((old_ranges, new_ranges)) = changed(&old.content, &new.content) {
                    if !old_ranges.is_empty() {
                        emphasis.removed.insert(old_number, old_ranges);
                    }
                    if !new_ranges.is_empty() {
                        emphasis.added.insert(new_number, new_ranges);
                    }
                }
            }
        }
    }
    emphasis
}

/// Byte ranges of the tokens of `line`: runs of letters, digits and `_`,
/// runs of whitespace, and every other character on its own.
pub fn tokens(line: &str) -> Vec<Range<usize>> {
    #[derive(PartialEq)]
    enum Class {
        Word,
        Space,
        Other,
    }
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            Class::Word
        } else if c.is_whitespace() {
            Class::Space
        } else {
            Class::Other
        }
    };
    let mut tokens: Vec<Range<usize>> = Vec::new();
    let mut previous = None;
    for (at, c) in line.char_indices() {
        let current = class(c);
        let joins = current != Class::Other && previous.as_ref() == Some(&current);
        match tokens.last_mut() {
            Some(last) if joins => last.end = at + c.len_utf8(),
            _ => tokens.push(at..at + c.len_utf8()),
        }
        previous = Some(current);
    }
    tokens
}

/// Changed byte ranges of the old and the new line.
pub type Changed = (Vec<Range<usize>>, Vec<Range<usize>>);

/// Changed byte ranges of `old` and `new` (adjacent changed tokens merged),
/// or `None` when the lines are too long or have too little in common.
pub fn changed(old: &str, new: &str) -> Option<Changed> {
    let (old_chars, new_chars) = (old.chars().count(), new.chars().count());
    if old_chars > MAX_LINE_CHARS || new_chars > MAX_LINE_CHARS {
        return None;
    }
    let old_tokens = tokens(old);
    let new_tokens = tokens(new);
    let a: Vec<&str> = old_tokens.iter().map(|r| &old[r.clone()]).collect();
    let b: Vec<&str> = new_tokens.iter().map(|r| &new[r.clone()]).collect();
    let (keep_a, keep_b) = common(&a, &b);
    let visible = |text: &str| text.chars().filter(|c| !c.is_whitespace()).count();
    let unchanged: usize = a
        .iter()
        .zip(&keep_a)
        .filter(|(_, keep)| **keep)
        .map(|(token, _)| visible(token))
        .sum();
    let total = visible(old) + visible(new);
    if total == 0 || (2 * unchanged) as f64 / (total as f64) < MIN_UNCHANGED {
        return None;
    }
    Some((merge(&old_tokens, &keep_a), merge(&new_tokens, &keep_b)))
}

/// Which tokens of `a` and `b` belong to their longest common sequence.
fn common(a: &[&str], b: &[&str]) -> (Vec<bool>, Vec<bool>) {
    let mut keep_a = vec![false; a.len()];
    let mut keep_b = vec![false; b.len()];
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    keep_a[..prefix].fill(true);
    keep_b[..prefix].fill(true);
    keep_a[a.len() - suffix..].fill(true);
    keep_b[b.len() - suffix..].fill(true);
    let mid_a = &a[prefix..a.len() - suffix];
    let mid_b = &b[prefix..b.len() - suffix];
    let (n, m) = (mid_a.len(), mid_b.len());
    if n == 0 || m == 0 || (n + 1) * (m + 1) > MAX_CELLS {
        return (keep_a, keep_b);
    }
    // Lengths of the longest common sequence of the suffixes from (i, j).
    let width = m + 1;
    let mut table = vec![0u16; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i * width + j] = if mid_a[i] == mid_b[j] {
                table[(i + 1) * width + j + 1] + 1
            } else {
                table[(i + 1) * width + j].max(table[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if mid_a[i] == mid_b[j] {
            keep_a[prefix + i] = true;
            keep_b[prefix + j] = true;
            i += 1;
            j += 1;
        } else if table[(i + 1) * width + j] >= table[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    (keep_a, keep_b)
}

/// Byte ranges of the tokens not kept, adjacent ones merged.
fn merge(tokens: &[Range<usize>], keep: &[bool]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (token, kept) in tokens.iter().zip(keep) {
        if *kept {
            continue;
        }
        match ranges.last_mut() {
            Some(last) if last.end == token.start => last.end = token.end,
            _ => ranges.push(token.clone()),
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::{DiffHunk, DiffLine};

    fn texts<'a>(line: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
        ranges.iter().map(|r| &line[r.clone()]).collect()
    }

    #[test]
    fn an_added_visibility_is_one_span_on_the_new_side() {
        let old = "    fn view_mut(&mut self) -> Option<&mut RepoView> {";
        let new = "    pub(crate) fn view_mut(&mut self) -> Option<&mut RepoView> {";
        let (a, b) = changed(old, new).unwrap();
        assert!(a.is_empty());
        assert_eq!(texts(new, &b), ["pub(crate) "]);
    }

    #[test]
    fn a_renamed_identifier_is_one_span_per_side() {
        let old = "let count = items.len() + 1;";
        let new = "let total = items.len() + 1;";
        let (a, b) = changed(old, new).unwrap();
        assert_eq!(texts(old, &a), ["count"]);
        assert_eq!(texts(new, &b), ["total"]);
    }

    #[test]
    fn a_rewritten_line_gets_no_emphasis() {
        assert_eq!(
            changed("let x = compute(a, b);", "return Err(error.into());"),
            None
        );
        // Exactly 40 % unchanged is still shown, below is not: "ab"
        // stays, 2 of 5 visible characters per side.
        assert!(changed("ab cde", "ab fgh").is_some());
        assert_eq!(changed("ab cdef", "ab ghij"), None);
        // Whitespace does not count: the same indentation does not make
        // a rewritten line similar.
        assert_eq!(
            changed(
                "        self.tabs.clear();",
                "        drop(std::mem::take(&mut self.history));"
            ),
            None
        );
        // "a bcdefgh" vs "a ijklmno": 1 of 8 per side (12 %).
        assert_eq!(changed("a bcdefgh", "a ijklmno"), None);
    }

    #[test]
    fn long_lines_get_no_emphasis() {
        let long = "x".repeat(MAX_LINE_CHARS + 1);
        assert_eq!(changed(&long, &format!("{long} y")), None);
        let limit = "é".repeat(MAX_LINE_CHARS - 2);
        // Characters count, not bytes.
        assert!(changed(&format!("{limit} a"), &format!("{limit} b")).is_some());
    }

    #[test]
    fn tabs_and_unicode_keep_char_boundaries() {
        let old = "\tlet größe = \"ä\";";
        let new = "\tlet größe = \"ö\"; // ü";
        let (a, b) = changed(old, new).unwrap();
        assert_eq!(texts(old, &a), ["ä"]);
        assert_eq!(texts(new, &b), ["ö", " // ü"]);
        for (line, ranges) in [(old, &a), (new, &b)] {
            for r in ranges.iter() {
                assert!(line.is_char_boundary(r.start) && line.is_char_boundary(r.end));
            }
        }
        assert_eq!(
            texts("a\t\tb_1 + ü", &tokens("a\t\tb_1 + ü")),
            ["a", "\t\t", "b_1", " ", "+", " ", "ü"]
        );
    }

    fn line(kind: DiffLineKind, number: u32, content: &str) -> DiffLine {
        DiffLine {
            kind,
            content: content.into(),
            old_line: (kind != DiffLineKind::Added).then_some(number),
            new_line: (kind != DiffLineKind::Removed).then_some(number),
            missing_newline: false,
            crlf: false,
        }
    }

    fn diff(lines: Vec<DiffLine>) -> FileDiff {
        FileDiff {
            old_path: Some("a.rs".into()),
            new_path: Some("a.rs".into()),
            binary: false,
            too_large: false,
            encoding: None,
            hunks: vec![DiffHunk {
                header: String::new(),
                old_start: 1,
                old_count: 0,
                new_start: 1,
                new_count: 0,
                lines,
            }],
        }
    }

    #[test]
    fn blocks_pair_lines_in_order_and_skip_the_rest() {
        use DiffLineKind::{Added, Context, Removed};
        let diff = diff(vec![
            line(Context, 1, "fn a() {"),
            line(Removed, 2, "    let x = 1;"),
            line(Removed, 3, "    let y = 2;"),
            line(Added, 2, "    let x = 10;"),
            line(Context, 4, "}"),
            line(Added, 5, "    let z = 3;"),
            line(Removed, 5, "    reset_everything();"),
            line(Added, 6, "    let q = 4;"),
            line(Added, 7, "    let w = 5;"),
        ]);
        let emphasis = of_diff(&diff);
        let lines = &diff.hunks[0].lines;
        let shown = |i: usize| texts(&lines[i].content, emphasis.ranges(&lines[i]));
        // First block: two removed, one added; only the first pair.
        assert_eq!(shown(1), ["1"]);
        assert_eq!(shown(3), ["10"]);
        assert!(shown(2).is_empty());
        // An added line after a context line has no partner.
        assert!(shown(5).is_empty());
        // "reset_everything();" vs "let q = 4;": too little in common
        // (27 %), no emphasis; the
        // extra added line has none either.
        assert!(shown(6).is_empty() && shown(7).is_empty() && shown(8).is_empty());
        assert!(shown(0).is_empty() && shown(4).is_empty());
        // Removed and added lines are told apart by their kind.
        assert!(!emphasis.is_empty());
    }
}
