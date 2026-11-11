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

/// The syntect theme family of a Gitloom theme (ADR-047): Dark and Classic
/// Dark use "OneHalfDark", Light uses "GitHub". Gitloom keeps its own
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

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

fn theme(theme: CodeTheme) -> &'static Theme {
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
fn syntax_for(path: &str, first_line: &str) -> Option<&'static SyntaxReference> {
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

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The licenses of the bundled syntaxes and themes, as listed in the
/// third-party notices (`crates/app/assets/syntax-acknowledgements.md`),
/// with LF line ends like every file in the repository.
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
