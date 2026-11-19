//! Minimum sizes of the panes and the window (finding #45, ADR-056), in
//! logical pixels at 100 % zoom. The window minimum is derived from the
//! pane minimums; it is fixed (independent of which panels are visible)
//! and scales with the zoom because iced applies the zoom to the logical
//! size given to `window::set_min_size`.

use iced::Size;

use crate::ui::panel::{
    EXTRA_LINE_HEIGHT, STAGING_BOTTOM_FIXED, STAGING_LISTS_MIN, STAGING_MESSAGE_MIN,
};

/// Left panel.
pub const SIDEBAR_MIN_WIDTH: f32 = 180.0;
/// Graph (or diff) in the center: at least the graph table's columns at
/// their minimum widths, see [`graph_columns_min_width`].
pub const CENTER_MIN_WIDTH: f32 = 480.0;
/// Details panel at the right.
pub const PANEL_MIN_WIDTH: f32 = 280.0;
/// Graph above the details panel at the bottom.
pub const BOTTOM_GRAPH_MIN_HEIGHT: f32 = 120.0;
/// Details panel at the bottom: the fixed rows of the staging area at the
/// bottom (header, toolbar, splitter, commit button), the lists' and the
/// message's minimums and two hint lines below the commit button (e.g.
/// "detached HEAD" and "Enter a commit summary").
pub const BOTTOM_PANEL_MIN_HEIGHT: f32 =
    STAGING_BOTTOM_FIXED + STAGING_LISTS_MIN + STAGING_MESSAGE_MIN + HINT_LINES;
/// Details panel at the right: the staging area (the taller of details and
/// staging) with both lists at two rows and two hint lines, the first of
/// which ("You are in detached HEAD; …") wraps at the panel's minimum
/// width (measured, finding #47).
pub const RIGHT_PANEL_MIN_HEIGHT: f32 =
    crate::ui::panel::STAGING_RIGHT_MIN + HINT_LINES + HINT_TEXT_LINE;
/// Two hint lines below the commit button.
const HINT_LINES: f32 = 2.0 * EXTRA_LINE_HEIGHT;
/// One more line of hint text (11 px) where a hint wraps.
const HINT_TEXT_LINE: f32 = 11.0 * 1.3;
/// Gap between panes (the pane grid's spacing).
pub const PANE_SPACING: f32 = 1.0;
/// Extra grab room of a pane divider (the pane grid's resize leeway): the
/// grab area is `PANE_SPACING + DIVIDER_LEEWAY` wide, centered on the gap.
pub const DIVIDER_LEEWAY: f32 = 6.0;
/// How far the grab area of a divider reaches into the panes on both
/// sides (finding #48).
pub const DIVIDER_REACH: f32 = DIVIDER_LEEWAY / 2.0;
/// Everything above and below the panes: tab bar, toolbar, status bar and
/// the gaps between them (measured, see the `chrome_height_is_measured`
/// UI check).
pub const CHROME_HEIGHT: f32 = 104.5;
/// The window is never narrower than this, even if the panes would fit.
const WINDOW_MIN_WIDTH: f32 = 1000.0;
/// Upper bound of the window minimum: fits 1366 × 768 with a task bar.
#[cfg(test)]
pub const WINDOW_MIN_LIMIT: Size = Size::new(1000.0, 680.0);

/// Sum of the graph table's column minimums (M11-02).
#[cfg(test)]
pub fn graph_columns_min_width() -> f32 {
    crate::graph_columns::ColumnId::ALL
        .iter()
        .map(|c| crate::graph_columns::min_width(*c, crate::ui::MIN_GRAPH_WIDTH))
        .sum()
}

/// Smallest window at 100 % zoom: all three panes side by side at their
/// minimum widths, and the taller of both panel positions.
pub fn window_min() -> Size {
    let panes = SIDEBAR_MIN_WIDTH + CENTER_MIN_WIDTH + PANEL_MIN_WIDTH + 2.0 * PANE_SPACING;
    let bottom = BOTTOM_GRAPH_MIN_HEIGHT + PANE_SPACING + BOTTOM_PANEL_MIN_HEIGHT;
    let height = CHROME_HEIGHT + bottom.max(RIGHT_PANEL_MIN_HEIGHT);
    Size::new(panes.max(WINDOW_MIN_WIDTH).ceil(), height.ceil())
}

/// [`window_min`] limited to `monitor` (logical pixels of the current
/// zoom, as `window::monitor_size` reports them): at high zoom on a small
/// screen the window may get smaller than its minimum rather than larger
/// than the screen.
pub fn window_min_on(monitor: Option<Size>) -> Size {
    let min = window_min();
    match monitor {
        Some(monitor) if monitor.width > 0.0 && monitor.height > 0.0 => Size::new(
            min.width.min(monitor.width.floor()),
            min.height.min(monitor.height.floor()),
        ),
        _ => min,
    }
}

/// Room of the panes in a window `window` large (logical pixels).
pub fn pane_area(window: Size) -> Size {
    Size::new(window.width, (window.height - CHROME_HEIGHT).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_minimum_follows_from_the_panes() {
        let min = window_min();
        assert!(
            min.width
                >= SIDEBAR_MIN_WIDTH + CENTER_MIN_WIDTH + PANEL_MIN_WIDTH + 2.0 * PANE_SPACING
        );
        assert!(min.height >= CHROME_HEIGHT + RIGHT_PANEL_MIN_HEIGHT);
        assert!(
            min.height
                >= CHROME_HEIGHT + BOTTOM_GRAPH_MIN_HEIGHT + PANE_SPACING + BOTTOM_PANEL_MIN_HEIGHT
        );
        assert!(
            min.width <= WINDOW_MIN_LIMIT.width && min.height <= WINDOW_MIN_LIMIT.height,
            "{min:?} must fit 1366 × 768 with a task bar"
        );
        assert_eq!(min.width, 1000.0);
        // The staging area at the right with its framed lists decides the
        // height (finding #47); RELEASE-CHECKLIST 7m names this size.
        assert_eq!(min.height, 668.0, "{min:?}");
    }

    #[test]
    fn center_keeps_the_graph_columns() {
        assert!(
            CENTER_MIN_WIDTH >= graph_columns_min_width(),
            "{CENTER_MIN_WIDTH} < {}",
            graph_columns_min_width()
        );
    }

    #[test]
    fn minimum_is_limited_to_the_monitor() {
        let min = window_min();
        assert_eq!(window_min_on(None), min);
        assert_eq!(window_min_on(Some(Size::new(3000.0, 2000.0))), min);
        // 200 % zoom on 1366 × 768: 683 × 384 logical pixels.
        assert_eq!(
            window_min_on(Some(Size::new(683.0, 384.0))),
            Size::new(683.0, 384.0)
        );
        assert_eq!(window_min_on(Some(Size::ZERO)), min);
        // 125 % zoom on 1366 × 768: the height is limited, the width not.
        let monitor = Size::new(1366.0 / 1.25, 768.0 / 1.25);
        assert_eq!(window_min_on(Some(monitor)), Size::new(1000.0, 614.0));
    }
}
