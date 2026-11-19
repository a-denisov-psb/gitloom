//! Syntax highlighting engine (M11-08, ADR-047): `syntect` with the syntax
//! and theme sets of `two-face`. Pure: no iced types. Whole files are
//! highlighted (so multi-line comments and strings are right on every
//! line) and the result is mapped to diff lines by the views (M11-09).
//!
//! Highlighting is CPU work: callers run it in a background task and keep
//! complete results in a [`Cache`] keyed by blob id and theme. Files of any
//! length are highlighted progressively (fix after M15, ADR-047): syntect
//! parses from the top, so [`Progressive`] works in blocks of
//! [`BLOCK_LINES`] lines and [`run`] hands out the lines done so far after
//! every block; the rest stays plain until its block is done.

use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::sync::{Arc, OnceLock, Weak};

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use two_face::theme::EmbeddedThemeName;

/// Files larger than this stay plain ("Too large for syntax highlighting").
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// Lines longer than this (minified or generated data) stay plain; the
/// parser continues on the next line with its state from before the line.
pub const MAX_LINE_BYTES: usize = 5_000;
/// Lines highlighted per block before the partial result is handed out.
pub const BLOCK_LINES: usize = 1_000;

/// The embedded `two-face` theme a Gitloom theme takes its syntax colors
/// from (ADR-047, ADR-058): Dark uses "OneHalfDark", Light "GitHub".
/// Gitloom keeps its own backgrounds; only the text color and style come
/// from the theme. Part of the cache key and of every job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CodeTheme(pub EmbeddedThemeName);

impl CodeTheme {
    /// The pairing of the built-in Dark theme.
    pub const DARK: Self = Self(EmbeddedThemeName::OneHalfDark);
    /// The pairing of the built-in Light theme.
    pub const LIGHT: Self = Self(EmbeddedThemeName::Github);

    fn embedded(self) -> EmbeddedThemeName {
        self.0
    }

    /// The name of the theme in the set (stored in theme files, M17).
    #[allow(dead_code)] // Used by theme files (M17).
    pub fn name(self) -> &'static str {
        self.0.as_name()
    }

    /// The embedded theme called `name`.
    #[allow(dead_code)] // Used by theme files (M17).
    pub fn named(name: &str) -> Option<Self> {
        two_face::theme::EmbeddedLazyThemeSet::theme_names()
            .iter()
            .find(|t| t.as_name() == name)
            .map(|t| Self(*t))
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

/// Spans of the lines of a file highlighted so far, in blocks of
/// [`BLOCK_LINES`] lines (a partial result shares the finished blocks
/// instead of copying them). Lines after the last block stay plain.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Highlighted {
    blocks: Vec<Arc<Vec<Vec<Span>>>>,
    /// Every line of the file is done.
    complete: bool,
}

impl Highlighted {
    /// Spans of line `index` (0-based), `None` when it is not done yet.
    pub fn line(&self, index: usize) -> Option<&[Span]> {
        self.blocks
            .get(index / BLOCK_LINES)?
            .get(index % BLOCK_LINES)
            .map(Vec::as_slice)
    }

    /// Lines done.
    #[cfg(test)]
    pub fn lines_done(&self) -> usize {
        self.blocks.iter().map(|b| b.len()).sum()
    }

    /// Every line of the file is done (only complete results are cached).
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Why a file is shown without highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plain {
    /// No syntax for the file type.
    UnknownType,
    /// Over [`MAX_BYTES`]: the view says "Too large for syntax
    /// highlighting".
    TooLarge,
}

/// Result of highlighting a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Highlighted(Arc<Highlighted>),
    Plain(Plain),
}

impl Outcome {
    /// Spans of line `index` (0-based) when it is highlighted.
    pub fn line(&self, index: usize) -> Option<&[Span]> {
        match self {
            Self::Highlighted(h) => h.line(index),
            Self::Plain(_) => None,
        }
    }

    /// Final: plain, or every line done.
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Highlighted(h) => h.is_complete(),
            Self::Plain(_) => true,
        }
    }
}

pub(crate) fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

pub(crate) fn theme(theme: CodeTheme) -> &'static Theme {
    static SET: OnceLock<two_face::theme::EmbeddedLazyThemeSet> = OnceLock::new();
    SET.get_or_init(two_face::theme::extra)
        .get(theme.embedded())
}

/// Name of the syntax without highlighting; as a language override it
/// switches highlighting off for the file (ADR-057).
pub const PLAIN_TEXT: &str = "Plain Text";

/// The syntax called `name` in the bundled set (any letter case is not
/// matched: names are stored as the set spells them).
pub fn syntax_named(name: &str) -> Option<&'static SyntaxReference> {
    syntaxes().find_syntax_by_name(name)
}

/// Names of the languages a file can be set to (ADR-057): every syntax of
/// the bundled set that is not hidden, sorted without regard to case,
/// without "Plain Text" (the list offers it on its own). Computed once.
pub fn language_names() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names: Vec<&'static str> = syntaxes()
            .syntaxes()
            .iter()
            .filter(|syntax| !syntax.hidden && syntax.name != PLAIN_TEXT)
            .map(|syntax| syntax.name.as_str())
            .collect();
        names.sort_by_cached_key(|name| (name.to_lowercase(), *name));
        names.dedup();
        names
    })
}

/// A language override that names a syntax: "Plain Text" or a syntax of
/// the set. Unknown names (for example after a `two-face` update) are
/// treated like no choice.
pub fn known_language(language: Option<&str>) -> Option<&str> {
    language.filter(|name| *name == PLAIN_TEXT || syntax_named(name).is_some())
}

/// The syntax of `path` (by extension, then by file name such as
/// `Makefile`), else by the first line (`#!/usr/bin/env python`); `None`
/// for plain text and unknown types.
fn detect(path: &str, first_line: &str) -> Option<&'static SyntaxReference> {
    let set = syntaxes();
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let by_extension = name
        .rsplit_once('.')
        .and_then(|(_, extension)| set.find_syntax_by_extension(extension));
    by_extension
        .or_else(|| set.find_syntax_by_extension(name))
        .or_else(|| set.find_syntax_by_first_line(first_line))
        .filter(|syntax| syntax.name != PLAIN_TEXT)
}

/// The syntax a file is highlighted with: the language chosen for it
/// (ADR-057; "Plain Text" means none, an unknown name counts as no
/// choice), else what [`detect`] finds.
pub(crate) fn syntax_for(
    path: &str,
    first_line: &str,
    language: Option<&str>,
) -> Option<&'static SyntaxReference> {
    match known_language(language) {
        Some(PLAIN_TEXT) => None,
        Some(name) => syntax_named(name),
        None => detect(path, first_line),
    }
}

/// Name of the syntax [`syntax_for`] picks, "Plain Text" without one: what
/// the status bar shows for the file (ADR-057).
pub fn syntax_name(path: &str, first_line: &str, language: Option<&str>) -> &'static str {
    syntax_for(path, first_line, language).map_or(PLAIN_TEXT, |syntax| syntax.name.as_str())
}

/// A file to highlight: its path, its text and the language chosen for it
/// (ADR-057; `None`: detected from path and first line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub path: String,
    pub text: String,
    pub language: Option<String>,
}

impl Source {
    pub fn new(path: impl Into<String>, text: impl Into<String>, language: Option<&str>) -> Self {
        Self {
            path: path.into(),
            text: text.into(),
            language: language.map(str::to_owned),
        }
    }

    /// Name of the syntax detection finds for the file (without the
    /// chosen language): "Auto Detect" in the language list.
    pub fn detected(&self) -> &'static str {
        syntax_name(&self.path, first_line(&self.text), None)
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// Highlights a file block by block: [`Progressive::step`] does the next
/// [`BLOCK_LINES`] lines, [`Progressive::outcome`] is the result so far.
pub struct Progressive {
    text: Arc<str>,
    /// Byte offset of the next line.
    at: usize,
    /// `None`: plain (the outcome is final).
    highlighter: Option<HighlightLines<'static>>,
    blocks: Vec<Arc<Vec<Vec<Span>>>>,
    plain: Plain,
}

impl Progressive {
    /// Starts highlighting `text` of the file at `path` with `theme_id`,
    /// in `language` when one was chosen for the file (ADR-057).
    pub fn new(path: &str, text: Arc<str>, theme_id: CodeTheme, language: Option<&str>) -> Self {
        let mut job = Self {
            text,
            at: 0,
            highlighter: None,
            blocks: Vec::new(),
            plain: Plain::UnknownType,
        };
        if job.text.len() > MAX_BYTES {
            job.plain = Plain::TooLarge;
            return job;
        }
        if let Some(syntax) = syntax_for(path, first_line(&job.text), language) {
            job.highlighter = Some(HighlightLines::new(syntax, theme(theme_id)));
        }
        job
    }

    /// Every line is done (or the file stays plain).
    pub fn done(&self) -> bool {
        self.highlighter.is_none() || self.at >= self.text.len()
    }

    /// Highlights the next block; `true` when the file is done.
    pub fn step(&mut self) -> bool {
        let Some(highlighter) = self.highlighter.as_mut() else {
            return true;
        };
        let rest = &self.text[self.at..];
        let mut block = Vec::with_capacity(BLOCK_LINES);
        let mut consumed = 0;
        let mut failed = false;
        // The "newlines" syntax set expects every line with its terminator.
        for line in syntect::util::LinesWithEndings::from(rest).take(BLOCK_LINES) {
            consumed += line.len();
            let text_len = line.trim_end_matches(['\n', '\r']).len();
            if failed || text_len > MAX_LINE_BYTES {
                // Plain; the parser state from before this line carries on.
                block.push(Vec::new());
                continue;
            }
            match highlighter.highlight_line(line, syntaxes()) {
                Ok(ranges) => block.push(spans_of(&ranges, text_len)),
                Err(_) => {
                    // A grammar error: the rest of the file stays plain.
                    failed = true;
                    block.push(Vec::new());
                }
            }
        }
        self.at += consumed;
        if !block.is_empty() {
            self.blocks.push(Arc::new(block));
        }
        if failed {
            // Plain lines up to the end, then the job is done.
            while self.at < self.text.len() {
                let lines = syntect::util::LinesWithEndings::from(&self.text[self.at..])
                    .take(BLOCK_LINES)
                    .fold((0, 0), |(n, bytes), l| (n + 1, bytes + l.len()));
                self.at += lines.1;
                self.blocks.push(Arc::new(vec![Vec::new(); lines.0]));
            }
        }
        self.done()
    }

    /// The result so far: the finished blocks, or why the file stays plain.
    pub fn outcome(&self) -> Outcome {
        if self.highlighter.is_none() {
            return Outcome::Plain(self.plain);
        }
        Outcome::Highlighted(Arc::new(Highlighted {
            blocks: self.blocks.clone(),
            complete: self.done(),
        }))
    }
}

/// Spans of one line from syntect's ranges, cut at the line's text length.
fn spans_of(ranges: &[(syntect::highlighting::Style, &str)], text_len: usize) -> Vec<Span> {
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
    spans
}

/// Highlights `content` of the file at `path` with `theme` in one go
/// (tests and the few lines of the theme editor's preview).
pub fn highlight(path: &str, content: &str, theme_id: CodeTheme) -> Outcome {
    highlight_in(path, content, theme_id, None)
}

/// [`highlight`] in a chosen language (ADR-057).
pub fn highlight_in(
    path: &str,
    content: &str,
    theme_id: CodeTheme,
    language: Option<&str>,
) -> Outcome {
    let mut job = Progressive::new(path, Arc::from(content), theme_id, language);
    while !job.step() {}
    job.outcome()
}

/// A running highlighting job, kept by the view it highlights for. The job
/// stops when every copy of its handle is gone: the view was replaced or
/// moved on to other content, another theme or another language.
#[derive(Debug, Clone)]
pub struct Job {
    alive: Arc<()>,
    pub theme: CodeTheme,
    /// The language chosen for the file when the job started (ADR-057).
    pub language: Option<String>,
}

impl Job {
    pub fn new(theme: CodeTheme, language: Option<&str>) -> Self {
        Self {
            alive: Arc::new(()),
            theme,
            language: language.map(str::to_owned),
        }
    }

    /// The job highlights with `theme` in `language`.
    pub fn is_for(&self, theme: CodeTheme, language: Option<&str>) -> bool {
        self.theme == theme && self.language.as_deref() == language
    }

    /// What the background task checks between lines.
    pub fn watch(&self) -> Watch {
        Watch(Arc::downgrade(&self.alive))
    }
}

/// The background side of a [`Job`].
#[derive(Debug, Clone)]
pub struct Watch(Weak<()>);

impl Watch {
    pub fn cancelled(&self) -> bool {
        self.0.strong_count() == 0
    }
}

/// Highlights `files` (path, text and chosen language) with `theme`:
/// complete results from
/// `cache`, the others block by block, each file on its own thread (top
/// first in every file). `emit` gets the outcomes of all files after every
/// block; the last call has every outcome complete. Nothing is emitted
/// once `watch` is cancelled, and only complete results go into the cache.
pub fn run(
    cache: &std::sync::Mutex<Cache>,
    files: Vec<Source>,
    theme: CodeTheme,
    watch: &Watch,
    mut emit: impl FnMut(Vec<Outcome>),
) {
    let mut ids = Vec::new();
    let mut current = Vec::new();
    let mut jobs = Vec::new();
    for (index, source) in files.into_iter().enumerate() {
        let id = content_id(&source.path, &source.text, source.language.as_deref());
        match cache.lock().ok().and_then(|c| c.get(&id, theme).cloned()) {
            Some(outcome) => current.push(outcome),
            None => {
                // Not done yet: plain until the first block arrives.
                current.push(Outcome::Highlighted(Arc::default()));
                jobs.push((index, source));
            }
        }
        ids.push(id);
    }
    if watch.cancelled() {
        return;
    }
    if jobs.is_empty() {
        emit(current);
        return;
    }
    let mut fresh = vec![false; current.len()];
    let finished = std::thread::scope(|scope| {
        let (sender, blocks) = std::sync::mpsc::channel();
        for (index, source) in jobs {
            fresh[index] = true;
            let sender = sender.clone();
            // The parser state stays on its thread (oniguruma's is not Send).
            scope.spawn(move || {
                let mut job = Progressive::new(
                    &source.path,
                    Arc::from(source.text),
                    theme,
                    source.language.as_deref(),
                );
                loop {
                    if watch.cancelled() {
                        return;
                    }
                    let done = job.step();
                    if sender.send((index, job.outcome())).is_err() || done {
                        return;
                    }
                }
            });
        }
        drop(sender);
        // Ends when every thread is done (or stopped).
        for (index, outcome) in blocks {
            if watch.cancelled() {
                return false;
            }
            current[index] = outcome;
            emit(current.clone());
        }
        !watch.cancelled()
    });
    if !finished {
        return;
    }
    if let Ok(mut cache) = cache.lock() {
        for ((id, outcome), fresh) in ids.iter().zip(current).zip(fresh) {
            if fresh && outcome.is_complete() {
                cache.insert(id, theme, outcome);
            }
        }
    }
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
    /// The language chosen for the file (ADR-057), `None`: detected.
    pub language: Option<String>,
    /// What detection finds for the file (the new side, or the old side
    /// when the new one has no syntax, such as a deleted script).
    pub detected: &'static str,
    pub old: Outcome,
    pub new: Outcome,
}

impl DiffSides {
    /// Highlighted with `theme` in `language`.
    pub fn is_for(&self, theme: CodeTheme, language: Option<&str>) -> bool {
        self.theme == theme && self.language.as_deref() == language
    }

    /// Spans of a diff line: removed lines from the old side, added and
    /// context lines from the new side (by their line numbers).
    pub fn spans_for(&self, line: &git_core::DiffLine) -> Option<&[Span]> {
        let (side, number) = match line.kind {
            git_core::DiffLineKind::Removed => (&self.old, line.old_line),
            git_core::DiffLineKind::Added | git_core::DiffLineKind::Context => {
                (&self.new, line.new_line)
            }
        };
        let index = usize::try_from(number?.checked_sub(1)?).ok()?;
        side.line(index)
    }

    /// The file was too large on a side (the view says so).
    pub fn too_large(&self) -> bool {
        [&self.old, &self.new]
            .iter()
            .any(|side| **side == Outcome::Plain(Plain::TooLarge))
    }
}

/// What detection finds for a file shown from several texts (the sides of
/// a diff or a conflict): the first that has a syntax, else "Plain Text".
pub fn detected_of(names: &[&'static str]) -> &'static str {
    names
        .iter()
        .copied()
        .find(|name| *name != PLAIN_TEXT)
        .unwrap_or(PLAIN_TEXT)
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

/// An id for a file's content and the language chosen for it (stands in
/// for its blob id in the cache; another language is another entry).
pub fn content_id(path: &str, text: &str, language: Option<&str>) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    text.hash(&mut hasher);
    language.hash(&mut hasher);
    format!("{:016x}-{}", hasher.finish(), text.len())
}

/// `content` cut into styled pieces by `spans` (unstyled between them),
/// with tabs expanded to `tab` columns as the views show them.
pub fn segments(content: &str, spans: &[Span], tab: usize) -> Vec<(String, Option<SpanStyle>)> {
    pieces(content, spans, &[], tab)
        .into_iter()
        .map(|piece| (piece.text, piece.style))
        .collect()
}

/// A piece of a line as shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    pub style: Option<SpanStyle>,
    /// Inside a changed range of the word diff (fix after M15).
    pub emphasis: bool,
}

/// `content` cut into pieces at the borders of `spans` and of the changed
/// byte ranges `emphasis` (word diff), with tabs expanded to `tab` columns.
/// Ranges outside the line or not on character borders are skipped.
pub fn pieces(content: &str, spans: &[Span], emphasis: &[Range<usize>], tab: usize) -> Vec<Piece> {
    let valid = |range: &Range<usize>| {
        range.start < range.end
            && range.end <= content.len()
            && content.is_char_boundary(range.start)
            && content.is_char_boundary(range.end)
    };
    // Styled ranges in order, without overlaps.
    let mut styled: Vec<(Range<usize>, SpanStyle)> = Vec::new();
    for span in spans {
        let start = span
            .range
            .start
            .max(styled.last().map_or(0, |(r, _)| r.end));
        let range = start..span.range.end.min(content.len());
        if valid(&range) {
            styled.push((range, span.style));
        }
    }
    let emphasis: Vec<&Range<usize>> = emphasis.iter().filter(|r| valid(r)).collect();
    let mut borders: Vec<usize> = vec![0, content.len()];
    for (range, _) in &styled {
        borders.extend([range.start, range.end]);
    }
    for range in &emphasis {
        borders.extend([range.start, range.end]);
    }
    borders.sort_unstable();
    borders.dedup();
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
    let mut out = Vec::new();
    let mut next_style = 0;
    for pair in borders.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        while styled.get(next_style).is_some_and(|(r, _)| r.end <= start) {
            next_style += 1;
        }
        let style = styled
            .get(next_style)
            .filter(|(r, _)| r.start <= start)
            .map(|(_, style)| *style);
        out.push(Piece {
            text: expand(&content[start..end]),
            style,
            emphasis: emphasis.iter().any(|r| r.start <= start && end <= r.end),
        });
    }
    out
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
        h.line(line)
            .unwrap()
            .iter()
            .find(|s| s.range.contains(&at))
            .unwrap_or_else(|| panic!("no span at {at} in {content:?}"))
            .style
    }

    #[test]
    fn rust_keywords_strings_and_comments_differ() {
        let source = "fn main() {\n    // greet\n    let s = \"hi\";\n}\n";
        let outcome = highlight("src/main.rs", source, CodeTheme::DARK);
        let h = spans(&outcome);
        assert_eq!(h.lines_done(), 4);
        assert!(h.is_complete());
        let keyword = style_of(source, h, 0, "fn");
        let comment = style_of(source, h, 1, "greet");
        let string = style_of(source, h, 2, "hi");
        let plain = style_of(source, h, 2, "s =");
        assert_ne!(keyword.color, plain.color);
        assert_ne!(comment.color, plain.color);
        assert_ne!(string.color, plain.color);
        assert_ne!(string.color, comment.color);
        // Spans stay inside the line text.
        for (index, content) in source.lines().enumerate() {
            let line = h.line(index).unwrap();
            assert!(line.iter().all(|s| s.range.end <= content.len()));
        }
    }

    #[test]
    fn python_by_extension_and_by_shebang() {
        let source = "def f():\n    return 'x'  # done\n";
        let h = highlight("tool.py", source, CodeTheme::LIGHT);
        let h = spans(&h);
        assert_ne!(
            style_of(source, h, 0, "def").color,
            style_of(source, h, 1, "    ").color
        );
        let script = "#!/usr/bin/env python3\nprint('x')\n";
        assert!(matches!(
            highlight("bin/tool", script, CodeTheme::DARK),
            Outcome::Highlighted(_)
        ));
        // A file name without extension.
        assert!(matches!(
            highlight("Makefile", "all:\n\techo hi\n", CodeTheme::DARK),
            Outcome::Highlighted(_)
        ));
    }

    #[test]
    fn multi_line_comment_keeps_its_style() {
        // Line 3 alone looks like code; inside the comment it is not.
        let source = "int a = 1;\n/* start\n   int b = 2;\n   end */\nint c = 3;\n";
        let outcome = highlight("x.c", source, CodeTheme::DARK);
        let h = spans(&outcome);
        let comment = style_of(source, h, 1, "start");
        assert_eq!(style_of(source, h, 2, "int b"), comment);
        assert_eq!(style_of(source, h, 3, "end"), comment);
        assert_ne!(style_of(source, h, 4, "int"), comment);
    }

    #[test]
    fn unknown_types_and_large_files_stay_plain() {
        assert_eq!(
            highlight("notes.unknownext", "hello\n", CodeTheme::DARK),
            Outcome::Plain(Plain::UnknownType)
        );
        assert_eq!(
            highlight("README", "hello\n", CodeTheme::DARK),
            Outcome::Plain(Plain::UnknownType)
        );
        let wide = "a".repeat(MAX_BYTES + 1);
        assert_eq!(
            highlight("a.rs", &wide, CodeTheme::DARK),
            Outcome::Plain(Plain::TooLarge)
        );
    }

    #[test]
    fn themes_differ() {
        let source = "fn main() {}\n";
        let dark = highlight("a.rs", source, CodeTheme::DARK);
        let light = highlight("a.rs", source, CodeTheme::LIGHT);
        assert_ne!(dark, light);
    }

    #[test]
    fn cache_hits_by_blob_and_theme() {
        let mut cache = Cache::default();
        let outcome = highlight("a.rs", "fn x() {}\n", CodeTheme::DARK);
        cache.insert("blob1", CodeTheme::DARK, outcome.clone());
        assert_eq!(cache.get("blob1", CodeTheme::DARK), Some(&outcome));
        assert!(cache.get("blob1", CodeTheme::LIGHT).is_none());
        // Every syntax theme has its own entries (ADR-058).
        let monokai = CodeTheme(EmbeddedThemeName::MonokaiExtended);
        assert!(cache.get("blob1", monokai).is_none());
        cache.insert("blob1", monokai, outcome.clone());
        assert_eq!(cache.get("blob1", monokai), Some(&outcome));
        assert!(cache.get("blob2", CodeTheme::DARK).is_none());
        // The same key again does not grow the cache.
        cache.insert("blob1", CodeTheme::DARK, outcome.clone());
        assert_eq!(cache.len(), 2);
        // The oldest entries go first.
        for i in 0..Cache::CAPACITY {
            cache.insert(&format!("b{i}"), CodeTheme::DARK, outcome.clone());
        }
        assert_eq!(cache.len(), Cache::CAPACITY);
        assert!(cache.get("blob1", CodeTheme::DARK).is_none());
        assert!(cache.get("b63", CodeTheme::DARK).is_some());
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
            theme: CodeTheme::DARK,
            language: None,
            detected: "Rust",
            old: highlight("a.rs", &old, CodeTheme::DARK),
            new: highlight("a.rs", &new, CodeTheme::DARK),
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
    fn pieces_split_at_emphasis_and_keep_syntax_colors() {
        let style = SpanStyle {
            color: [1, 2, 3, 255],
            bold: false,
            italic: false,
        };
        // "pub(crate) fn x" with ") fn" styled and "pub(crate) " emphasized.
        let content = "pub(crate) fn x";
        let spans = [Span {
            range: 9..13,
            style,
        }];
        let emphasized = [Range { start: 0, end: 11 }];
        let pieces = pieces(content, &spans, &emphasized, 4);
        let shown: Vec<(&str, bool, bool)> = pieces
            .iter()
            .map(|p| (p.text.as_str(), p.style.is_some(), p.emphasis))
            .collect();
        assert_eq!(
            shown,
            [
                ("pub(crate", false, true),
                (") ", true, true),
                ("fn", true, false),
                (" x", false, false),
            ]
        );
        // Tabs expand across pieces; ranges off char borders are skipped.
        let pieces = super::pieces("\tä\tb", &[], &[1..2, 3..5], 4);
        let texts: Vec<&str> = pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, ["    ä", "   b"]);
        assert!(pieces[1].emphasis && !pieces[0].emphasis);
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

    /// The old one-shot algorithm (before the fix after M15): one
    /// highlighter over the whole file, no line limit.
    fn reference(path: &str, content: &str) -> Vec<Vec<Span>> {
        let syntax = syntax_for(path, content.lines().next().unwrap_or(""), None).unwrap();
        let mut highlighter = HighlightLines::new(syntax, theme(CodeTheme::DARK));
        syntect::util::LinesWithEndings::from(content)
            .map(|line| {
                let ranges = highlighter.highlight_line(line, syntaxes()).unwrap();
                spans_of(&ranges, line.trim_end_matches(['\n', '\r']).len())
            })
            .collect()
    }

    fn all_lines(h: &Highlighted) -> Vec<Vec<Span>> {
        (0..h.lines_done())
            .map(|i| h.line(i).unwrap().to_vec())
            .collect()
    }

    /// A Rust file whose block comments and raw strings cross the block
    /// borders (line 1000 is inside a comment, line 2000 inside a string).
    fn crossing_blocks() -> String {
        let mut source = String::new();
        for i in 0..2_600 {
            match i {
                995 => source.push_str("/* a comment over the block border\n"),
                1_004 => source.push_str("   end of comment */\n"),
                1_998 => source.push_str("const S: &str = r#\"a raw string\n"),
                2_003 => source.push_str("still the string\"#;\n"),
                _ => source.push_str(&format!("fn f{i}() -> u32 {{ {i} }} // c\n")),
            }
        }
        source
    }

    #[test]
    fn progressive_result_equals_one_shot() {
        let source = crossing_blocks();
        let mut job = Progressive::new("a.rs", Arc::from(source.as_str()), CodeTheme::DARK, None);
        let mut partials = Vec::new();
        while !job.step() {
            partials.push(job.outcome());
        }
        let Outcome::Highlighted(last) = job.outcome() else {
            panic!("plain");
        };
        assert!(last.is_complete());
        assert_eq!(all_lines(&last), reference("a.rs", &source));
        // Partial results: whole blocks, top first, never complete, and
        // each line already as in the final result.
        assert_eq!(partials.len(), 2);
        for (n, partial) in partials.iter().enumerate() {
            let Outcome::Highlighted(h) = partial else {
                panic!("plain");
            };
            assert!(!h.is_complete());
            assert_eq!(h.lines_done(), (n + 1) * BLOCK_LINES);
            assert!(h.line(h.lines_done()).is_none());
            for i in 0..h.lines_done() {
                assert_eq!(h.line(i), last.line(i));
            }
        }
        // The comment and the string continue over the borders.
        let comment = last.line(996).unwrap()[0].style;
        assert!(last.line(1_000).unwrap().iter().all(|s| s.style == comment));
        assert_ne!(last.line(1_010).unwrap()[0].style, comment);
        let string = last.line(1_999).unwrap()[0].style;
        assert!(last.line(2_000).unwrap().iter().all(|s| s.style == string));
        assert_eq!(last.line(2_001).unwrap()[0].style, string);
        assert_ne!(string, last.line(2_010).unwrap()[0].style);
    }

    #[test]
    fn long_lines_stay_plain_and_the_next_line_is_right() {
        let long = format!("let s = \"{}\";", "x".repeat(MAX_LINE_BYTES));
        let source = format!("/* open\n{long}\nstill */ fn after() {{}}\nfn g() {{}}\n");
        let outcome = highlight("a.rs", &source, CodeTheme::DARK);
        let h = spans(&outcome);
        assert_eq!(h.lines_done(), 4);
        assert_eq!(h.line(1), Some(&[][..]));
        // The parser state from before the long line continues: line 3
        // still ends the comment opened on line 1.
        let comment = style_of(&source, h, 0, "open");
        assert_eq!(style_of(&source, h, 2, "still"), comment);
        assert_ne!(style_of(&source, h, 2, "fn"), comment);
        assert_ne!(style_of(&source, h, 3, "fn"), comment);
        // Exactly the limit is still highlighted.
        let at_limit = format!("// {}\n", "y".repeat(MAX_LINE_BYTES - 3));
        let outcome = highlight("a.rs", &at_limit, CodeTheme::DARK);
        assert!(!spans(&outcome).line(0).unwrap().is_empty());
    }

    #[test]
    fn no_line_limit_but_a_size_limit() {
        // More lines than the old limit of 20,000 (the 100,000-line file
        // is measured in release: `perf_highlight_and_wrap`).
        let many = "fn x() {}\n".repeat(30_000);
        let outcome = highlight("a.rs", &many, CodeTheme::DARK);
        let h = spans(&outcome);
        assert_eq!(h.lines_done(), 30_000);
        assert!(!h.line(29_999).unwrap().is_empty());
        let huge = "x\n".repeat(MAX_BYTES / 2 + 1);
        assert_eq!(
            highlight("a.rs", &huge, CodeTheme::DARK),
            Outcome::Plain(Plain::TooLarge)
        );
    }

    #[test]
    fn run_emits_rounds_and_caches_complete_results() {
        let cache = std::sync::Mutex::new(Cache::default());
        let source = crossing_blocks();
        let job = Job::new(CodeTheme::DARK, None);
        let mut emitted = Vec::new();
        let files = vec![
            Source::new("a.rs", source.clone(), None),
            Source::new("b.rs", "fn b() {}\n", None),
        ];
        run(&cache, files.clone(), CodeTheme::DARK, &job.watch(), |o| {
            emitted.push(o)
        });
        // Three blocks of a.rs and one of b.rs, each handed out at once;
        // a.rs top first.
        assert_eq!(emitted.len(), 4);
        let done_a: Vec<usize> = emitted
            .iter()
            .map(|o| match &o[0] {
                Outcome::Highlighted(h) => h.lines_done(),
                Outcome::Plain(_) => panic!("plain"),
            })
            .collect();
        assert!(done_a.windows(2).all(|w| w[0] <= w[1]), "{done_a:?}");
        let last = emitted.last().unwrap();
        assert!(last.iter().all(Outcome::is_complete));
        assert_eq!(cache.lock().unwrap().len(), 2);
        // From the cache: one call, the same result.
        let mut again = Vec::new();
        run(&cache, files, CodeTheme::DARK, &job.watch(), |o| {
            again.push(o)
        });
        assert_eq!(again, std::slice::from_ref(last));
        assert_ne!(content_id("a.rs", "x", None), content_id("b.rs", "x", None));
    }

    #[test]
    fn a_cancelled_run_emits_nothing_stale_and_caches_nothing() {
        let cache = std::sync::Mutex::new(Cache::default());
        let mut job = Some(Job::new(CodeTheme::DARK, None));
        let watch = job.as_ref().unwrap().watch();
        let mut emitted = 0;
        let files = vec![Source::new("a.rs", crossing_blocks(), None)];
        run(&cache, files.clone(), CodeTheme::DARK, &watch, |_| {
            emitted += 1;
            // The view moves on after the first block.
            job = None;
        });
        assert!(job.is_none());
        assert_eq!(emitted, 1);
        assert_eq!(cache.lock().unwrap().len(), 0);
        // A job dropped before it starts emits nothing at all; a clone of
        // the handle keeps it alive.
        let job = Job::new(CodeTheme::DARK, None);
        let copy = job.clone();
        let watch = job.watch();
        drop(job);
        assert!(!watch.cancelled());
        drop(copy);
        let mut any = false;
        run(&cache, files, CodeTheme::DARK, &watch, |_| any = true);
        assert!(!any);
    }

    const GODOT: &str = "; Engine configuration file.\n\n[application]\nconfig/name=\"Game\"\n";

    #[test]
    fn a_chosen_language_highlights_an_unknown_file_type() {
        // ADR-057: `project.godot` is INI, but its extension is unknown.
        assert_eq!(
            highlight("project.godot", GODOT, CodeTheme::DARK),
            Outcome::Plain(Plain::UnknownType)
        );
        let outcome = highlight_in("project.godot", GODOT, CodeTheme::DARK, Some("INI"));
        let h = spans(&outcome);
        assert_ne!(
            style_of(GODOT, h, 0, "Engine").color,
            style_of(GODOT, h, 2, "application").color
        );
        // The same as the file under an INI name.
        assert_eq!(outcome, highlight("project.ini", GODOT, CodeTheme::DARK));
    }

    #[test]
    fn plain_text_switches_highlighting_off_and_unknown_names_are_ignored() {
        let source = "fn main() {}\n";
        assert_eq!(
            highlight_in("a.rs", source, CodeTheme::DARK, Some(PLAIN_TEXT)),
            Outcome::Plain(Plain::UnknownType)
        );
        // A name the bundled set does not have works like no choice.
        assert_eq!(
            highlight_in("a.rs", source, CodeTheme::DARK, Some("No Such Language")),
            highlight("a.rs", source, CodeTheme::DARK)
        );
        assert_eq!(known_language(Some("No Such Language")), None);
        assert_eq!(known_language(Some(PLAIN_TEXT)), Some(PLAIN_TEXT));
        assert_eq!(known_language(Some("INI")), Some("INI"));
    }

    #[test]
    fn the_cache_key_changes_with_the_language() {
        let ids = [None, Some("INI"), Some(PLAIN_TEXT)]
            .map(|language| content_id("project.godot", GODOT, language));
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
        assert_ne!(ids[0], ids[2]);
        // `run` keeps one entry per language.
        let cache = std::sync::Mutex::new(Cache::default());
        let job = Job::new(CodeTheme::DARK, Some("INI"));
        for language in [None, Some("INI")] {
            let files = vec![Source::new("project.godot", GODOT, language)];
            let mut last = None;
            run(&cache, files, CodeTheme::DARK, &job.watch(), |o| {
                last = Some(o)
            });
            let plain = matches!(last.unwrap()[0], Outcome::Plain(_));
            assert_eq!(plain, language.is_none());
        }
        assert_eq!(cache.lock().unwrap().len(), 2);
        assert!(job.is_for(CodeTheme::DARK, Some("INI")));
        assert!(!job.is_for(CodeTheme::DARK, None));
        assert!(!job.is_for(CodeTheme::LIGHT, Some("INI")));
    }

    #[test]
    fn language_names_are_sorted_visible_and_unique() {
        let names = language_names();
        assert!(names.contains(&"INI"));
        assert!(names.contains(&"Rust"));
        assert!(!names.contains(&PLAIN_TEXT));
        let hidden: Vec<&str> = syntaxes()
            .syntaxes()
            .iter()
            .filter(|s| s.hidden)
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.iter().all(|n| !hidden.contains(n) || {
            // A hidden syntax may share its name with a visible one.
            syntaxes()
                .syntaxes()
                .iter()
                .any(|s| s.name == *n && !s.hidden)
        }));
        assert!(names
            .windows(2)
            .all(|w| w[0].to_lowercase() <= w[1].to_lowercase() && w[0] != w[1]));
        // Every name can be chosen.
        assert!(names.iter().all(|n| syntax_named(n).is_some()));
    }

    #[test]
    fn the_shown_name_is_the_syntax_in_use() {
        assert_eq!(syntax_name("src/main.rs", "", None), "Rust");
        assert_eq!(syntax_name("Makefile", "", None), "Makefile");
        assert_eq!(
            syntax_name("bin/tool", "#!/usr/bin/env python3", None),
            "Python"
        );
        assert_eq!(syntax_name("project.godot", "", None), PLAIN_TEXT);
        assert_eq!(syntax_name("project.godot", "", Some("INI")), "INI");
        assert_eq!(syntax_name("a.rs", "", Some(PLAIN_TEXT)), PLAIN_TEXT);
        assert_eq!(syntax_name("a.rs", "", Some("Nope")), "Rust");
        // Detection ignores the chosen language.
        let source = Source::new("bin/tool", "#!/bin/sh\necho\n", Some("INI"));
        assert_eq!(source.detected(), "Bourne Again Shell (bash)");
        assert_eq!(detected_of(&[PLAIN_TEXT, "Rust"]), "Rust");
        assert_eq!(detected_of(&[PLAIN_TEXT]), PLAIN_TEXT);
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

    #[test]
    fn code_themes_by_name() {
        assert_eq!(CodeTheme::named("OneHalfDark"), Some(CodeTheme::DARK));
        assert_eq!(CodeTheme::named("GitHub"), Some(CodeTheme::LIGHT));
        assert_eq!(CodeTheme::DARK.name(), "OneHalfDark");
        assert_eq!(CodeTheme::named("No Such Theme"), None);
        // Other syntax themes color the same file differently.
        let source = "fn main() { let x = 1; }\n";
        let monokai = CodeTheme(EmbeddedThemeName::MonokaiExtended);
        assert_ne!(
            highlight("a.rs", source, CodeTheme::DARK),
            highlight("a.rs", source, monokai)
        );
    }
}
