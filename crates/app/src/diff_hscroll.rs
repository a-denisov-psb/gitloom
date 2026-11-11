//! Horizontal scrolling of the diff text (M7-16): content width, the
//! scrollable range of each layout and the mapping of wheel input. Pure
//! functions; the view and `App::update` use them.

use git_core::FileDiff;
use iced::mouse::ScrollDelta;

use crate::editor_style::EditorStyle;
use crate::repo_view::DiffLayout;

/// Room kept free at the right: the vertical scrollbar floats there.
pub const SCROLLBAR_ROOM: f32 = 12.0;
/// Room after the end of the longest line when scrolled to the end.
pub const END_PADDING: f32 = 12.0;
/// Pixels per wheel line, as iced's scrollable uses for vertical scrolling.
pub const PIXELS_PER_LINE: f32 = 60.0;
/// Suffix shown after a line without a newline at the end of the file.
pub const NO_NEWLINE: &str = "  (no newline at end of file)";

/// The line of `diff` with the most characters, as shown (tabs expanded
/// to `tab`, with the no-newline note).
pub fn longest_line(diff: &FileDiff, tab: usize) -> String {
    diff.hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .map(|line| {
            let mut text = crate::editor_style::expand_tabs(&line.content, tab).into_owned();
            if line.missing_newline {
                text.push_str(NO_NEWLINE);
            }
            text
        })
        .max_by_key(|text| text.chars().count())
        .unwrap_or_default()
}

/// Width of the code text whose longest line is `line` in the editor
/// font, plus [`END_PADDING`] (measured once per loaded diff or style).
pub fn text_width(line: &str, style: &EditorStyle) -> f32 {
    if line.is_empty() {
        return 0.0;
    }
    measure(line, style) + END_PADDING
}

/// Width of one character in the (monospace) editor font.
pub fn char_width(style: &EditorStyle) -> f32 {
    measure(&"0".repeat(20), style) / 20.0
}

/// Width of `text` in the editor font.
fn measure(text: &str, style: &EditorStyle) -> f32 {
    use iced::advanced::text::Paragraph as _;
    let paragraph = iced::advanced::graphics::text::Paragraph::with_text(iced::advanced::Text {
        content: text,
        bounds: iced::Size::INFINITE,
        size: style.size.into(),
        line_height: iced::widget::text::LineHeight::default(),
        font: style.font,
        align_x: iced::advanced::text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: iced::widget::text::Shaping::default(),
        wrapping: iced::widget::text::Wrapping::None,
    });
    paragraph.min_bounds().width
}

/// Width left of the code text in `layout`: line numbers and marker (one
/// side for Split).
pub fn gutter(layout: DiffLayout, style: &EditorStyle) -> f32 {
    let (number, marker) = (style.number_width(), style.marker_width());
    match layout {
        DiffLayout::Split => number + marker,
        DiffLayout::Hunks | DiffLayout::Inline => 2.0 * number + marker,
    }
}

/// Visible width of the code text in a diff `width` wide: each half of a
/// Split diff clips its own text.
pub fn text_area(layout: DiffLayout, width: f32, style: &EditorStyle) -> f32 {
    let gutter = gutter(layout, style);
    let area = match layout {
        // Two halves and a 1 px divider; the right half has the scrollbar.
        DiffLayout::Split => (width - 1.0 - SCROLLBAR_ROOM) / 2.0 - gutter,
        DiffLayout::Hunks | DiffLayout::Inline => width - gutter - SCROLLBAR_ROOM,
    };
    area.max(0.0)
}

/// Largest horizontal offset: the longest line ends at the right edge.
pub fn max_offset(layout: DiffLayout, text_width: f32, width: f32, style: &EditorStyle) -> f32 {
    (text_width - text_area(layout, width, style)).max(0.0)
}

/// Horizontal pixels a wheel or touchpad event scrolls (positive = to the
/// right), or `None` for purely vertical scrolling. Shift turns a vertical
/// wheel horizontal like iced's scrollable does (on macOS the system
/// already swaps the axes while Shift is held).
pub fn wheel_to_horizontal(delta: ScrollDelta, shift: bool, macos: bool) -> Option<f32> {
    let pixels = match delta {
        ScrollDelta::Lines { x, y } => {
            let horizontal = if shift && !macos { y } else { x };
            -horizontal * PIXELS_PER_LINE
        }
        ScrollDelta::Pixels { x, .. } => -x,
    };
    (pixels != 0.0).then_some(pixels)
}

/// `offset` moved by `delta`, kept within `0..=max`.
pub fn scrolled(offset: f32, delta: f32, max: f32) -> f32 {
    (offset + delta).clamp(0.0, max.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::{DiffHunk, DiffLine, DiffLineKind};

    fn diff(lines: &[(&str, bool)]) -> FileDiff {
        FileDiff {
            old_path: Some("a".into()),
            new_path: Some("a".into()),
            binary: false,
            too_large: false,
            encoding: None,
            hunks: vec![DiffHunk {
                header: "@@ -1 +1 @@".into(),
                old_start: 1,
                old_count: 1,
                new_start: 1,
                new_count: 1,
                lines: lines
                    .iter()
                    .map(|(content, missing_newline)| DiffLine {
                        kind: DiffLineKind::Added,
                        content: (*content).into(),
                        old_line: None,
                        new_line: Some(1),
                        missing_newline: *missing_newline,
                        crlf: false,
                    })
                    .collect(),
            }],
        }
    }

    fn width(diff: &FileDiff) -> f32 {
        let style = EditorStyle::default();
        text_width(&longest_line(diff, style.tab), &style)
    }

    #[test]
    fn width_follows_the_longest_line() {
        let short = diff(&[("ab", false)]);
        let long = diff(&[("ab", false), (&"x".repeat(200), false)]);
        assert_eq!(longest_line(&long, 4).len(), 200);
        assert!(width(&long) > width(&short) * 10.0);
        // Monospace: twice the characters, twice the width.
        let double = diff(&[(&"x".repeat(400), false)]);
        let one = width(&long) - END_PADDING;
        let two = width(&double) - END_PADDING;
        assert!((two - 2.0 * one).abs() < 1.0, "{one} {two}");
        // The no-newline note counts.
        let note = diff(&[("ab", true)]);
        assert_eq!(longest_line(&note, 4), format!("ab{NO_NEWLINE}"));
        assert_eq!(width(&diff(&[])), 0.0);
        // Tabs count as spaces up to the tab stop.
        let tabbed = diff(&[("\tx", false), ("abc", false)]);
        assert_eq!(longest_line(&tabbed, 4), "    x");
        assert_eq!(longest_line(&tabbed, 2), "abc");
    }

    #[test]
    fn range_per_layout() {
        let style = EditorStyle::default();
        let width = 1000.0;
        let hunks = text_area(DiffLayout::Hunks, width, &style);
        assert_eq!(hunks, 1000.0 - 120.0 - SCROLLBAR_ROOM);
        assert_eq!(text_area(DiffLayout::Inline, width, &style), hunks);
        let split = text_area(DiffLayout::Split, width, &style);
        assert_eq!(split, (1000.0 - 1.0 - SCROLLBAR_ROOM) / 2.0 - 68.0);
        // Fits: no scrolling.
        assert_eq!(max_offset(DiffLayout::Hunks, 500.0, width, &style), 0.0);
        assert_eq!(
            max_offset(DiffLayout::Hunks, hunks + 40.0, width, &style),
            40.0
        );
        // The same text scrolls further in a half.
        assert!(
            max_offset(DiffLayout::Split, 900.0, width, &style)
                > max_offset(DiffLayout::Hunks, 900.0, width, &style)
        );
        assert_eq!(text_area(DiffLayout::Split, 10.0, &style), 0.0);
        // Hidden line numbers leave the marker only.
        let bare = EditorStyle {
            numbers: false,
            ..style
        };
        assert_eq!(gutter(DiffLayout::Hunks, &bare), 16.0);
    }

    #[test]
    fn clamping() {
        assert_eq!(scrolled(0.0, -60.0, 100.0), 0.0);
        assert_eq!(scrolled(80.0, 60.0, 100.0), 100.0);
        assert_eq!(scrolled(10.0, 5.0, 100.0), 15.0);
        assert_eq!(scrolled(50.0, 0.0, -3.0), 0.0);
    }

    #[test]
    fn wheel_mapping() {
        let lines = |x, y| ScrollDelta::Lines { x, y };
        // Plain vertical wheel: not horizontal.
        assert_eq!(wheel_to_horizontal(lines(0.0, -1.0), false, false), None);
        // Shift + wheel down scrolls right, up scrolls left.
        assert_eq!(
            wheel_to_horizontal(lines(0.0, -1.0), true, false),
            Some(60.0)
        );
        assert_eq!(
            wheel_to_horizontal(lines(0.0, 1.0), true, false),
            Some(-60.0)
        );
        // macOS delivers Shift + wheel as a horizontal delta.
        assert_eq!(
            wheel_to_horizontal(lines(-1.0, 0.0), true, true),
            Some(60.0)
        );
        assert_eq!(wheel_to_horizontal(lines(0.0, -1.0), true, true), None);
        // Horizontal wheel / tilt.
        assert_eq!(
            wheel_to_horizontal(lines(-2.0, 0.0), false, false),
            Some(120.0)
        );
        // Touchpad: the horizontal pixels, vertical ones are left to the
        // vertical scrollable.
        let pixels = ScrollDelta::Pixels { x: -7.5, y: 30.0 };
        assert_eq!(wheel_to_horizontal(pixels, false, false), Some(7.5));
        let vertical = ScrollDelta::Pixels { x: 0.0, y: 30.0 };
        assert_eq!(wheel_to_horizontal(vertical, true, false), None);
    }
}
