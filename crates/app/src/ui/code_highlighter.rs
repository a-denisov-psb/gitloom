//! Syntax highlighting of the editable conflict output (M11-09): the
//! highlighting engine of `crate::highlight` behind iced's text editor
//! highlighter interface. Parser states are kept every
//! [`LINES_PER_SNAPSHOT`] lines, so an edit re-highlights from the nearest
//! snapshot above it.

use std::ops::Range;

use iced::advanced::text::highlighter::{self, Highlighter};
use syntect::highlighting::{HighlightState, RangedHighlightIterator};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference};

use crate::highlight::{self, CodeTheme, SpanStyle};

const LINES_PER_SNAPSHOT: usize = 50;

/// What the editor highlights: the file path decides the syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub path: String,
    pub theme: CodeTheme,
    /// Preferences → Editor → Syntax highlighting.
    pub enabled: bool,
}

pub struct SyntaxHighlighter {
    syntax: Option<&'static SyntaxReference>,
    highlighter: syntect::highlighting::Highlighter<'static>,
    /// Parser and highlighter state at every snapshot line.
    snapshots: Vec<(ParseState, HighlightState)>,
    state: Option<(ParseState, HighlightState)>,
    current_line: usize,
}

impl SyntaxHighlighter {
    fn initial(&self) -> Option<(ParseState, HighlightState)> {
        let syntax = self.syntax?;
        Some((
            ParseState::new(syntax),
            HighlightState::new(&self.highlighter, ScopeStack::new()),
        ))
    }
}

impl Highlighter for SyntaxHighlighter {
    type Settings = Settings;
    type Highlight = SpanStyle;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, SpanStyle)>;

    fn new(settings: &Settings) -> Self {
        let syntax = settings
            .enabled
            .then(|| highlight::syntax_for(&settings.path, ""))
            .flatten();
        let mut new = Self {
            syntax,
            highlighter: syntect::highlighting::Highlighter::new(highlight::theme(settings.theme)),
            snapshots: Vec::new(),
            state: None,
            current_line: 0,
        };
        new.state = new.initial();
        new
    }

    fn update(&mut self, new_settings: &Settings) {
        *self = Self::new(new_settings);
    }

    fn change_line(&mut self, line: usize) {
        let snapshot = line / LINES_PER_SNAPSHOT;
        if snapshot < self.snapshots.len() {
            self.state = Some(self.snapshots[snapshot].clone());
            self.snapshots.truncate(snapshot);
            self.current_line = snapshot * LINES_PER_SNAPSHOT;
        } else if line < self.current_line {
            // Before any snapshot was taken there: from the top.
            self.snapshots.clear();
            self.state = self.initial();
            self.current_line = 0;
        }
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        if self.current_line.is_multiple_of(LINES_PER_SNAPSHOT) {
            if let Some(state) = &self.state {
                self.snapshots.push(state.clone());
            }
        }
        self.current_line += 1;
        let Some((parser, highlight_state)) = self.state.as_mut() else {
            return Vec::new().into_iter();
        };
        // The syntax set expects lines with their terminator.
        let text = format!("{line}\n");
        let Ok(ops) = parser.parse_line(&text, highlight::syntaxes()) else {
            return Vec::new().into_iter();
        };
        RangedHighlightIterator::new(highlight_state, &ops, &text, &self.highlighter)
            .filter_map(|(style, _, range)| {
                let range = range.start..range.end.min(line.len());
                (range.start < range.end).then(|| {
                    let c = style.foreground;
                    (
                        range,
                        SpanStyle {
                            color: [c.r, c.g, c.b, c.a],
                            bold: false,
                            italic: false,
                        },
                    )
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// Text color of a highlight in the editor.
pub fn format(span: &SpanStyle, _theme: &iced::Theme) -> highlighter::Format<iced::Font> {
    let [r, g, b, a] = span.color;
    highlighter::Format {
        color: Some(iced::Color::from_rgba8(r, g, b, f32::from(a) / 255.0)),
        font: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(path: &str) -> Settings {
        Settings {
            path: path.into(),
            theme: CodeTheme::Dark,
            enabled: true,
        }
    }

    #[test]
    fn highlights_lines_with_state_across_lines() {
        let mut h = SyntaxHighlighter::new(&settings("a.rs"));
        let first: Vec<_> = h.highlight_line("/* start").collect();
        let second: Vec<_> = h.highlight_line("fn inside() {}").collect();
        let comment = first[0].1;
        assert!(second.iter().all(|(_, s)| *s == comment), "{second:?}");
        assert_eq!(h.current_line(), 2);
        // An edit of line 0 starts over from the top.
        h.change_line(0);
        assert_eq!(h.current_line(), 0);
        let again: Vec<_> = h.highlight_line("fn code() {}").collect();
        assert!(again.iter().any(|(_, s)| *s != comment));
    }

    #[test]
    fn unknown_type_or_switched_off_stays_plain() {
        let mut h = SyntaxHighlighter::new(&settings("notes.unknownext"));
        assert_eq!(h.highlight_line("fn x() {}").count(), 0);
        let mut off = SyntaxHighlighter::new(&Settings {
            enabled: false,
            ..settings("a.rs")
        });
        assert_eq!(off.highlight_line("fn x() {}").count(), 0);
    }

    #[test]
    fn snapshots_resume_below_an_edit() {
        let mut h = SyntaxHighlighter::new(&settings("a.rs"));
        for _ in 0..120 {
            let _ = h.highlight_line("let x = 1;").count();
        }
        h.change_line(75);
        assert_eq!(h.current_line(), 50);
    }
}
