//! Syntax highlighting engine (M11-08, ADR-047): `syntect` with the syntax
//! and theme sets of `two-face`. Pure: no iced types. Whole files are
//! highlighted (so multi-line comments and strings are right on every
//! line) and the result is mapped to diff lines by the views (M11-09).
//!
//! Highlighting is CPU work: callers run [`highlight`] in a background
//! task and keep results in a [`Cache`] keyed by blob id and theme.

use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use two_face::theme::EmbeddedThemeName;

/// Files larger than this stay plain.
pub const MAX_BYTES: usize = 1024 * 1024;
/// Files with more lines than this stay plain.
pub const MAX_LINES: usize = 20_000;

/// The syntect theme family of a Gitloom theme (ADR-047): Dark uses
/// "OneHalfDark", Light uses "GitHub". Gitloom keeps its own
/// backgrounds; only the text color and style come from the theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodeTheme {
    Dark,
    Light,
}

impl CodeTheme {
    fn embedded(self) -> EmbeddedThemeName {
        match self {
            Self::Dark => EmbeddedThemeName::OneHalfDark,
            Self::Light => EmbeddedThemeName::Github,
        }
    }
}

/// Text style of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanStyle {
    /// RGBA.
    pub color: [u8; 4],
    pub bold: bool,
    pub italic: bool,
}

/// A styled byte range of one line (without its line terminator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: SpanStyle,
}

/// Spans of every line of a file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Highlighted {
    pub lines: Vec<Vec<Span>>,
}

/// Why a file is shown without highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plain {
    /// No syntax for the file type.
    UnknownType,
    /// Over [`MAX_BYTES`] or [`MAX_LINES`]: the view says "Too large for
    /// syntax highlighting".
    TooLarge,
}

/// Result of highlighting a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Highlighted(Arc<Highlighted>),
    Plain(Plain),
}

pub(crate) fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

pub(crate) fn theme(theme: CodeTheme) -> &'static Theme {
    static DARK: OnceLock<Theme> = OnceLock::new();
    static LIGHT: OnceLock<Theme> = OnceLock::new();
    let cell = match theme {
        CodeTheme::Dark => &DARK,
        CodeTheme::Light => &LIGHT,
    };
    cell.get_or_init(|| two_face::theme::extra().get(theme.embedded()).clone())
}

/// The syntax of `path` (by extension, then by file name such as
/// `Makefile`), else by the first line (`#!/usr/bin/env python`); `None`
/// for plain text and unknown types.
pub(crate) fn syntax_for(path: &str, first_line: &str) -> Option<&'static SyntaxReference> {
    let set = syntaxes();
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let by_extension = name
        .rsplit_once('.')
        .and_then(|(_, extension)| set.find_syntax_by_extension(extension));
    by_extension
        .or_else(|| set.find_syntax_by_extension(name))
        .or_else(|| set.find_syntax_by_first_line(first_line))
        .filter(|syntax| syntax.name != "Plain Text")
}

/// Highlights `content` of the file at `path` with `theme`.
pub fn highlight(path: &str, content: &str, theme_id: CodeTheme) -> Outcome {
    if content.len() > MAX_BYTES || content.lines().count() > MAX_LINES {
        return Outcome::Plain(Plain::TooLarge);
    }
    let first_line = content.lines().next().unwrap_or("");
    let Some(syntax) = syntax_for(path, first_line) else {
        return Outcome::Plain(Plain::UnknownType);
    };
    let mut highlighter = HighlightLines::new(syntax, theme(theme_id));
    let mut lines = Vec::new();
    // The "newlines" syntax set expects every line with its terminator.
    for line in syntect::util::LinesWithEndings::from(content) {
        let Ok(ranges) = highlighter.highlight_line(line, syntaxes()) else {
            // A grammar error: the rest of the file stays plain.
            return Outcome::Plain(Plain::UnknownType);
        };
        let text_len = line.trim_end_matches(['\n', '\r']).len();
        let mut start = 0;
        let mut spans = Vec::new();
        for (style, piece) in ranges {
            let end = (start + piece.len()).min(text_len);
            if end > start {
                let color = style.foreground;
                spans.push(Span {
                    range: start..end,
                    style: SpanStyle {
                        color: [color.r, color.g, color.b, color.a],
                        bold: style.font_style.contains(FontStyle::BOLD),
                        italic: style.font_style.contains(FontStyle::ITALIC),
                    },
                });
            }
            start += piece.len();
        }
        lines.push(spans);
    }
    Outcome::Highlighted(Arc::new(Highlighted { lines }))
}

/// Recently highlighted files, keyed by blob id (or another id that
/// changes with the content) and theme; the oldest entries go first.
#[derive(Debug, Default)]
pub struct Cache {
    entries: HashMap<(String, CodeTheme), Outcome>,
    order: VecDeque<(String, CodeTheme)>,
}

impl Cache {
    /// Entries kept at most.
    pub const CAPACITY: usize = 64;

    pub fn get(&self, id: &str, theme: CodeTheme) -> Option<&Outcome> {
        self.entries.get(&(id.to_owned(), theme))
    }

    pub fn insert(&mut self, id: &str, theme: CodeTheme, outcome: Outcome) {
        let key = (id.to_owned(), theme);
        if self.entries.insert(key.clone(), outcome).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > Self::CAPACITY {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Highlighted old and new side of a diff (M11-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSides {
    pub theme: CodeTheme,
    pub old: Outcome,
    pub new: Outcome,
}

impl DiffSides {
    /// Spans of a diff line: removed lines from the old side, added and
    /// context lines from the new side (by their line numbers).
    pub fn spans_for(&self, line: &git_core::DiffLine) -> Option<&[Span]> {
        let (side, number) = match line.kind {
            git_core::DiffLineKind::Removed => (&self.old, line.old_line),
            git_core::DiffLineKind::Added | git_core::DiffLineKind::Context => {
                (&self.new, line.new_line)
            }
        };
        let Outcome::Highlighted(h) = side else {
            return None;
        };
        let index = usize::try_from(number?.checked_sub(1)?).ok()?;
        h.lines.get(index).map(Vec::as_slice)
    }

    /// The file was too large on a side (the view says so).
    pub fn too_large(&self) -> bool {
        [&self.old, &self.new]
            .iter()
            .any(|side| **side == Outcome::Plain(Plain::TooLarge))
    }
}

/// Old and new file text from a diff with full context: every line at its
/// line number (lines a diff with less context leaves out stay empty).
pub fn diff_texts(diff: &git_core::FileDiff) -> (String, String) {
    fn place<'a>(side: &mut Vec<&'a str>, number: Option<u32>, text: &'a str) {
        let Some(index) = number.and_then(|n| usize::try_from(n).ok()?.checked_sub(1)) else {
            return;
        };
        if side.len() <= index {
            side.resize(index + 1, "");
        }
        side[index] = text;
    }
    let mut old: Vec<&str> = Vec::new();
    let mut new: Vec<&str> = Vec::new();
    for line in diff.hunks.iter().flat_map(|h| &h.lines) {
        match line.kind {
            git_core::DiffLineKind::Removed => place(&mut old, line.old_line, &line.content),
            git_core::DiffLineKind::Added => place(&mut new, line.new_line, &line.content),
            git_core::DiffLineKind::Context => {
                place(&mut old, line.old_line, &line.content);
                place(&mut new, line.new_line, &line.content);
            }
        }
    }
    let join = |lines: Vec<&str>| {
        let mut text = lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text
    };
    (join(old), join(new))
}

/// The "ours" and "theirs" file of a conflicted text: the clean parts with
/// each side's conflict lines (as the conflict view shows them).
pub fn conflict_texts(text: &git_core::ConflictText) -> (String, String) {
    let mut ours = String::new();
    let mut theirs = String::new();
    let push = |side: &mut String, line: &str| {
        side.push_str(line.trim_end_matches(['\n', '\r']));
        side.push('\n');
    };
    for chunk in &text.chunks {
        match chunk {
            git_core::Chunk::Clean(lines) => {
                for line in lines {
                    push(&mut ours, line);
                    push(&mut theirs, line);
                }
            }
            git_core::Chunk::Conflict(hunk) => {
                hunk.ours.iter().for_each(|l| push(&mut ours, l));
                hunk.theirs.iter().for_each(|l| push(&mut theirs, l));
            }
        }
    }
    (ours, theirs)
}

/// An id for a file's content (stands in for its blob id in the cache).
pub fn content_id(path: &str, text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    text.hash(&mut hasher);
    format!("{:016x}-{}", hasher.finish(), text.len())
}

/// Highlights `text` of `path`, from `cache` when it was done before.
pub fn cached(
    cache: &std::sync::Mutex<Cache>,
    path: &str,
    text: &str,
    theme: CodeTheme,
) -> Outcome {
    let id = content_id(path, text);
    if let Some(hit) = cache.lock().ok().and_then(|c| c.get(&id, theme).cloned()) {
        return hit;
    }
    let outcome = highlight(path, text, theme);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(&id, theme, outcome.clone());
    }
    outcome
}

/// `content` cut into styled pieces by `spans` (unstyled between them),
/// with tabs expanded to `tab` columns as the views show them.
pub fn segments(content: &str, spans: &[Span], tab: usize) -> Vec<(String, Option<SpanStyle>)> {
    let tab = tab.max(1);
    let mut column = 0;
    let mut expand = |piece: &str| {
        let mut out = String::with_capacity(piece.len());
        for c in piece.chars() {
            if c == '\t' {
                let spaces = tab - column % tab;
                out.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
            } else {
                out.push(c);
                column += 1;
            }
        }
        out
    };
    let mut pieces = Vec::new();
    let mut at = 0;
    for span in spans {
        let (start, end) = (span.range.start.max(at), span.range.end.min(content.len()));
        if start >= end || !content.is_char_boundary(start) || !content.is_char_boundary(end) {
            continue;
        }
        if start > at {
            pieces.push((expand(&content[at..start]), None));
        }
        pieces.push((expand(&content[start..end]), Some(span.style)));
        at = end;
    }
    if at < content.len() {
        pieces.push((expand(&content[at..]), None));
    }
    pieces
}

/// The licenses of the bundled syntaxes and themes, as listed in the
/// third-party notices (`crates/app/assets/syntax-acknowledgements.md`),
/// with LF line ends like every file in the repository.
#[cfg(test)]
pub fn acknowledgements() -> String {
    two_face::acknowledgement::listing()
        .to_md()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(outcome: &Outcome) -> &Highlighted {
        match outcome {
            Outcome::Highlighted(h) => h,
            Outcome::Plain(reason) => panic!("plain: {reason:?}"),
        }
    }

    /// Style of the span covering `text` in line `line` of `source`.
    fn style_of(source: &str, h: &Highlighted, line: usize, text: &str) -> SpanStyle {
        let content = source.lines().nth(line).unwrap();
        let at = content.find(text).unwrap();
        h.lines[line]
            .iter()
            .find(|s| s.range.contains(&at))
            .unwrap_or_else(|| panic!("no span at {at} in {content:?}"))
            .style
    }

    #[test]
    fn rust_keywords_strings_and_comments_differ() {
        let source = "fn main() {\n    // greet\n    let s = \"hi\";\n}\n";
        let outcome = highlight("src/main.rs", source, CodeTheme::Dark);
        let h = spans(&outcome);
        assert_eq!(h.lines.len(), 4);
        let keyword = style_of(source, h, 0, "fn");
        let comment = style_of(source, h, 1, "greet");
        let string = style_of(source, h, 2, "hi");
        let plain = style_of(source, h, 2, "s =");
        assert_ne!(keyword.color, plain.color);
        assert_ne!(comment.color, plain.color);
        assert_ne!(string.color, plain.color);
        assert_ne!(string.color, comment.color);
        // Spans stay inside the line text.
        for (line, content) in h.lines.iter().zip(source.lines()) {
            assert!(line.iter().all(|s| s.range.end <= content.len()));
        }
    }

    #[test]
    fn python_by_extension_and_by_shebang() {
        let source = "def f():\n    return 'x'  # done\n";
        let h = highlight("tool.py", source, CodeTheme::Light);
        let h = spans(&h);
        assert_ne!(
            style_of(source, h, 0, "def").color,
            style_of(source, h, 1, "    ").color
        );
        let script = "#!/usr/bin/env python3\nprint('x')\n";
        assert!(matches!(
            highlight("bin/tool", script, CodeTheme::Dark),
            Outcome::Highlighted(_)
        ));
        // A file name without extension.
        assert!(matches!(
            highlight("Makefile", "all:\n\techo hi\n", CodeTheme::Dark),
            Outcome::Highlighted(_)
        ));
    }

    #[test]
    fn multi_line_comment_keeps_its_style() {
        // Line 3 alone looks like code; inside the comment it is not.
        let source = "int a = 1;\n/* start\n   int b = 2;\n   end */\nint c = 3;\n";
        let outcome = highlight("x.c", source, CodeTheme::Dark);
        let h = spans(&outcome);
        let comment = style_of(source, h, 1, "start");
        assert_eq!(style_of(source, h, 2, "int b"), comment);
        assert_eq!(style_of(source, h, 3, "end"), comment);
        assert_ne!(style_of(source, h, 4, "int"), comment);
    }

    #[test]
    fn unknown_types_and_large_files_stay_plain() {
        assert_eq!(
            highlight("notes.unknownext", "hello\n", CodeTheme::Dark),
            Outcome::Plain(Plain::UnknownType)
        );
        assert_eq!(
            highlight("README", "hello\n", CodeTheme::Dark),
            Outcome::Plain(Plain::UnknownType)
        );
        let big = "x\n".repeat(MAX_LINES + 1);
        assert_eq!(
            highlight("a.rs", &big, CodeTheme::Dark),
            Outcome::Plain(Plain::TooLarge)
        );
        let wide = "a".repeat(MAX_BYTES + 1);
        assert_eq!(
            highlight("a.rs", &wide, CodeTheme::Dark),
            Outcome::Plain(Plain::TooLarge)
        );
    }

    #[test]
    fn themes_differ() {
        let source = "fn main() {}\n";
        let dark = highlight("a.rs", source, CodeTheme::Dark);
        let light = highlight("a.rs", source, CodeTheme::Light);
        assert_ne!(dark, light);
    }

    #[test]
    fn cache_hits_by_blob_and_theme() {
        let mut cache = Cache::default();
        let outcome = highlight("a.rs", "fn x() {}\n", CodeTheme::Dark);
        cache.insert("blob1", CodeTheme::Dark, outcome.clone());
        assert_eq!(cache.get("blob1", CodeTheme::Dark), Some(&outcome));
        assert!(cache.get("blob1", CodeTheme::Light).is_none());
        assert!(cache.get("blob2", CodeTheme::Dark).is_none());
        // The same key again does not grow the cache.
        cache.insert("blob1", CodeTheme::Dark, outcome.clone());
        assert_eq!(cache.len(), 1);
        // The oldest entries go first.
        for i in 0..Cache::CAPACITY {
            cache.insert(&format!("b{i}"), CodeTheme::Dark, outcome.clone());
        }
        assert_eq!(cache.len(), Cache::CAPACITY);
        assert!(cache.get("blob1", CodeTheme::Dark).is_none());
        assert!(cache.get("b63", CodeTheme::Dark).is_some());
    }

    #[test]
    fn diff_lines_take_spans_of_their_side() {
        use git_core::{DiffHunk, DiffLine, DiffLineKind, FileDiff};
        let line = |kind, old, new, content: &str| DiffLine {
            kind,
            content: content.into(),
            old_line: old,
            new_line: new,
            missing_newline: false,
            crlf: false,
        };
        let diff = FileDiff {
            old_path: Some("a.rs".into()),
            new_path: Some("a.rs".into()),
            binary: false,
            too_large: false,
            encoding: None,
            hunks: vec![DiffHunk {
                header: String::new(),
                old_start: 1,
                old_count: 3,
                new_start: 1,
                new_count: 3,
                lines: vec![
                    line(DiffLineKind::Context, Some(1), Some(1), "/* open"),
                    line(DiffLineKind::Removed, Some(2), None, "end */ fn old() {}"),
                    line(DiffLineKind::Added, None, Some(2), "still comment */"),
                    line(DiffLineKind::Context, Some(3), Some(3), "fn x() {}"),
                ],
            }],
        };
        let (old, new) = diff_texts(&diff);
        assert_eq!(old, "/* open\nend */ fn old() {}\nfn x() {}\n");
        assert_eq!(new, "/* open\nstill comment */\nfn x() {}\n");
        let sides = DiffSides {
            theme: CodeTheme::Dark,
            old: highlight("a.rs", &old, CodeTheme::Dark),
            new: highlight("a.rs", &new, CodeTheme::Dark),
        };
        let lines = &diff.hunks[0].lines;
        let open = sides.spans_for(&lines[0]).unwrap()[0].style;
        // The added line continues the comment on the new side.
        assert!(sides
            .spans_for(&lines[2])
            .unwrap()
            .iter()
            .all(|s| s.style == open));
        // The removed line ends the comment on the old side; code follows.
        assert!(sides
            .spans_for(&lines[1])
            .unwrap()
            .iter()
            .any(|s| s.style != open));
        assert!(!sides.too_large());
    }

    #[test]
    fn segments_expand_tabs_and_keep_gaps() {
        let style = SpanStyle {
            color: [1, 2, 3, 255],
            bold: false,
            italic: false,
        };
        let spans = [Span { range: 0..2, style }, Span { range: 3..5, style }];
        let pieces = segments("ab\tcd ef", &spans, 4);
        let texts: Vec<&str> = pieces.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["ab", "  ", "cd", " ef"]);
        assert_eq!(pieces[0].1, Some(style));
        assert_eq!(pieces[1].1, None);
        // Out-of-range spans are skipped.
        let bad = [Span {
            range: 50..60,
            style,
        }];
        assert_eq!(segments("x", &bad, 4), [("x".to_owned(), None)]);
    }

    #[test]
    fn cached_highlights_once() {
        let cache = std::sync::Mutex::new(Cache::default());
        let first = cached(&cache, "a.rs", "fn x() {}\n", CodeTheme::Dark);
        assert_eq!(cache.lock().unwrap().len(), 1);
        let second = cached(&cache, "a.rs", "fn x() {}\n", CodeTheme::Dark);
        assert_eq!(first, second);
        assert_eq!(cache.lock().unwrap().len(), 1);
        assert_ne!(content_id("a.rs", "x"), content_id("b.rs", "x"));
    }

    /// The third-party notices include this file; regenerate it with
    /// `UPDATE_ACKNOWLEDGEMENTS=1 cargo test -p gitloom acknowledgements`.
    #[test]
    fn acknowledgements_file_is_current() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/syntax-acknowledgements.md"
        );
        let expected = acknowledgements();
        if std::env::var_os("UPDATE_ACKNOWLEDGEMENTS").is_some() {
            std::fs::write(path, &expected).unwrap();
        }
        let actual = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            actual == expected,
            "{path} is out of date: UPDATE_ACKNOWLEDGEMENTS=1 cargo test -p gitloom acknowledgements"
        );
    }
}
