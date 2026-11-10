//! Horizontal scrolling of the diff text (M7-16): content width, the
//! scrollable range of each layout and the mapping of wheel input. Pure
//! functions; the view and `App::update` use them.

use git_core::FileDiff;
use iced::mouse::ScrollDelta;

use crate::repo_view::DiffLayout;

/// Font size of the diff text.
pub const TEXT_SIZE: f32 = 12.0;
/// Width of a line number column.
pub const NUMBER_WIDTH: f32 = 52.0;
/// Width of the `+`/`-` marker column.
pub const MARKER_WIDTH: f32 = 16.0;
/// Room kept free at the right: the vertical scrollbar floats there.
pub const SCROLLBAR_ROOM: f32 = 12.0;
/// Room after the end of the longest line when scrolled to the end.
pub const END_PADDING: f32 = 12.0;
/// Pixels per wheel line, as iced's scrollable uses for vertical scrolling.
pub const PIXELS_PER_LINE: f32 = 60.0;
/// Suffix shown after a line without a newline at the end of the file.
pub const NO_NEWLINE: &str = "  (no newline at end of file)";

/// The line of `diff` with the most characters, as shown (with the
/// no-newline note).
pub fn longest_line(diff: &FileDiff) -> String {
    diff.hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .map(|line| {
            let mut text = line.content.clone();
            if line.missing_newline {
                text.push_str(NO_NEWLINE);
            }
            text
        })
        .max_by_key(|text| text.chars().count())
        .unwrap_or_default()
}

/// Width of the code text of `diff` in the monospace font: the longest
/// line measured once (per loaded diff) plus [`END_PADDING`].
pub fn text_width(diff: &FileDiff) -> f32 {
    let line = longest_line(diff);
    if line.is_empty() {
        return 0.0;
    }
    measure(&line) + END_PADDING
}

/// Width of `text` in the monospace diff font.
fn measure(text: &str) -> f32 {
    use iced::advanced::text::Paragraph as _;
    let paragraph = iced::advanced::graphics::text::Paragraph::with_text(iced::advanced::Text {
        content: text,
        bounds: iced::Size::INFINITE,
        size: TEXT_SIZE.into(),
        line_height: iced::widget::text::LineHeight::default(),
        font: iced::Font::MONOSPACE,
        align_x: iced::advanced::text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: iced::widget::text::Shaping::default(),
        wrapping: iced::widget::text::Wrapping::None,
    });
    paragraph.min_bounds().width
}

/// Width left of the code text in `layout`: line numbers and marker (one
/// side for Split).
pub fn gutter(layout: DiffLayout) -> f32 {
    match layout {
        DiffLayout::Split => NUMBER_WIDTH + MARKER_WIDTH,
        DiffLayout::Hunks | DiffLayout::Inline => 2.0 * NUMBER_WIDTH + MARKER_WIDTH,
    }
}

/// Visible width of the code text in a diff `width` wide: each half of a
/// Split diff clips its own text.
pub fn text_area(layout: DiffLayout, width: f32) -> f32 {
    let area = match layout {
        // Two halves and a 1 px divider; the right half has the scrollbar.
        DiffLayout::Split => (width - 1.0 - SCROLLBAR_ROOM) / 2.0 - gutter(layout),
        DiffLayout::Hunks | DiffLayout::Inline => width - gutter(layout) - SCROLLBAR_ROOM,
    };
    area.max(0.0)
}

/// Largest horizontal offset: the longest line ends at the right edge.
pub fn max_offset(layout: DiffLayout, text_width: f32, width: f32) -> f32 {
    (text_width - text_area(layout, width)).max(0.0)
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

    #[test]
    fn width_follows_the_longest_line() {
        let short = diff(&[("ab", false)]);
        let long = diff(&[("ab", false), (&"x".repeat(200), false)]);
        assert_eq!(longest_line(&long).len(), 200);
        assert!(text_width(&long) > text_width(&short) * 10.0);
        // Monospace: twice the characters, twice the width.
        let double = diff(&[(&"x".repeat(400), false)]);
        let one = text_width(&long) - END_PADDING;
        let two = text_width(&double) - END_PADDING;
        assert!((two - 2.0 * one).abs() < 1.0, "{one} {two}");
        // The no-newline note counts.
        let note = diff(&[("ab", true)]);
        assert_eq!(longest_line(&note), format!("ab{NO_NEWLINE}"));
        assert_eq!(text_width(&diff(&[])), 0.0);
    }

    #[test]
    fn range_per_layout() {
        let width = 1000.0;
        let hunks = text_area(DiffLayout::Hunks, width);
        assert_eq!(hunks, 1000.0 - 120.0 - SCROLLBAR_ROOM);
        assert_eq!(text_area(DiffLayout::Inline, width), hunks);
        let split = text_area(DiffLayout::Split, width);
        assert_eq!(split, (1000.0 - 1.0 - SCROLLBAR_ROOM) / 2.0 - 68.0);
        // Fits: no scrolling.
        assert_eq!(max_offset(DiffLayout::Hunks, 500.0, width), 0.0);
        assert_eq!(max_offset(DiffLayout::Hunks, hunks + 40.0, width), 40.0);
        // The same text scrolls further in a half.
        assert!(
            max_offset(DiffLayout::Split, 900.0, width)
                > max_offset(DiffLayout::Hunks, 900.0, width)
        );
        assert_eq!(text_area(DiffLayout::Split, 10.0), 0.0);
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
