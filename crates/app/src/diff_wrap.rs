//! Word wrap in the diff views (M11-07): how many visual lines each display
//! row takes and where it starts. With a monospace font the count follows
//! from the character count and the columns that fit, so only the visible
//! rows are built even with wrapping (prefix sums, rebuilt per width).
//! Pure: no iced types.

/// Characters of a display row: one count, or the old and new side of a
/// Split row. Headers and bars count 0 (one visual line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowChars {
    pub left: u32,
    pub right: u32,
}

impl RowChars {
    pub fn single(chars: usize) -> Self {
        Self {
            left: u32::try_from(chars).unwrap_or(u32::MAX),
            right: 0,
        }
    }

    pub fn split(left: usize, right: usize) -> Self {
        Self {
            left: u32::try_from(left).unwrap_or(u32::MAX),
            right: u32::try_from(right).unwrap_or(u32::MAX),
        }
    }
}

/// Characters that fit into `width` pixels of text with characters
/// `advance` pixels wide (at least 1).
pub fn columns(width: f32, advance: f32) -> usize {
    if advance <= 0.0 || !width.is_finite() {
        return 1;
    }
    ((width / advance).floor() as usize).max(1)
}

/// Visual lines of `chars` characters with `columns` per line (at least 1).
pub fn line_count(chars: u32, columns: usize) -> u32 {
    let columns = u32::try_from(columns.max(1)).unwrap_or(u32::MAX);
    chars.div_ceil(columns).max(1)
}

/// Start (in visual lines) of every row and the total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapLayout {
    /// `starts[i]`: first visual line of row `i`; `starts[n]`: total.
    starts: Vec<u32>,
}

impl WrapLayout {
    /// Layout of `rows` with `columns` characters per visual line; a Split
    /// row takes the larger line count of its two sides, so both stay
    /// aligned.
    pub fn build(rows: &[RowChars], columns: usize) -> Self {
        let mut starts = Vec::with_capacity(rows.len() + 1);
        let mut line = 0u32;
        starts.push(0);
        for row in rows {
            let lines = line_count(row.left, columns).max(line_count(row.right, columns));
            line = line.saturating_add(lines);
            starts.push(line);
        }
        Self { starts }
    }

    pub fn rows(&self) -> usize {
        self.starts.len() - 1
    }

    /// Visual lines of all rows.
    pub fn total(&self) -> u32 {
        self.starts.last().copied().unwrap_or(0)
    }

    /// First visual line of `row` (the total past the end).
    pub fn start(&self, row: usize) -> u32 {
        self.starts
            .get(row)
            .copied()
            .unwrap_or_else(|| self.total())
    }

    /// Visual lines of `row`.
    pub fn lines(&self, row: usize) -> u32 {
        self.start(row + 1).saturating_sub(self.start(row))
    }

    /// The row that contains visual line `line` (the last row past the
    /// end).
    pub fn row_at(&self, line: u32) -> usize {
        let after = self.starts.partition_point(|&s| s <= line);
        after.saturating_sub(1).min(self.rows().saturating_sub(1))
    }

    /// Rows `first..last` intersecting the viewport `offset..offset+height`
    /// (pixels, `line_height` per visual line), extended by `overscan`
    /// rows.
    pub fn visible(
        &self,
        offset: f32,
        height: f32,
        line_height: f32,
        overscan: usize,
    ) -> (usize, usize) {
        let rows = self.rows();
        if rows == 0 || line_height <= 0.0 {
            return (0, 0);
        }
        let top = (offset.max(0.0) / line_height).floor() as u32;
        let bottom = ((offset.max(0.0) + height.max(0.0)) / line_height).ceil() as u32;
        let first = self.row_at(top).saturating_sub(overscan);
        let last = (self.row_at(bottom.saturating_sub(1).max(top)) + 1 + overscan).min(rows);
        (first, last.max(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        assert_eq!(columns(100.0, 7.0), 14);
        assert_eq!(columns(3.0, 7.0), 1);
        assert_eq!(columns(100.0, 0.0), 1);
        assert_eq!(line_count(0, 10), 1);
        assert_eq!(line_count(10, 10), 1);
        assert_eq!(line_count(11, 10), 2);
        assert_eq!(line_count(35, 10), 4);
        assert_eq!(line_count(5, 0), 5);
    }

    #[test]
    fn prefix_sums() {
        let rows = [
            RowChars::single(0),
            RowChars::single(25),
            RowChars::single(3),
            RowChars::single(10),
        ];
        let layout = WrapLayout::build(&rows, 10);
        assert_eq!(layout.rows(), 4);
        assert_eq!(layout.total(), 1 + 3 + 1 + 1);
        assert_eq!(
            (0..4).map(|r| layout.start(r)).collect::<Vec<_>>(),
            [0, 1, 4, 5]
        );
        assert_eq!(layout.lines(1), 3);
        assert_eq!(layout.row_at(0), 0);
        assert_eq!(layout.row_at(2), 1);
        assert_eq!(layout.row_at(3), 1);
        assert_eq!(layout.row_at(4), 2);
        assert_eq!(layout.row_at(99), 3);
    }

    #[test]
    fn resize_changes_the_counts() {
        let rows = [RowChars::single(30), RowChars::single(30)];
        assert_eq!(WrapLayout::build(&rows, 10).total(), 6);
        assert_eq!(WrapLayout::build(&rows, 15).total(), 4);
        assert_eq!(WrapLayout::build(&rows, 30).total(), 2);
        assert_eq!(WrapLayout::build(&rows, 1000).total(), 2);
    }

    #[test]
    fn split_rows_take_the_longer_side() {
        let rows = [
            RowChars::split(5, 25),
            RowChars::split(31, 0),
            RowChars::split(0, 0),
        ];
        let layout = WrapLayout::build(&rows, 10);
        assert_eq!(layout.lines(0), 3);
        assert_eq!(layout.lines(1), 4);
        assert_eq!(layout.lines(2), 1);
    }

    #[test]
    fn visible_rows() {
        // Rows of 1, 3, 1, 1, ... visual lines (20 px each).
        let mut rows = vec![RowChars::single(5), RowChars::single(25)];
        rows.extend(std::iter::repeat_n(RowChars::single(1), 50));
        let layout = WrapLayout::build(&rows, 10);
        // Lines 0..5 visible: rows 0 (line 0), 1 (1..4), 2 (4).
        assert_eq!(layout.visible(0.0, 100.0, 20.0, 0), (0, 3));
        // Scrolled into the wrapped row.
        assert_eq!(layout.visible(40.0, 20.0, 20.0, 0), (1, 2));
        assert_eq!(layout.visible(40.0, 20.0, 20.0, 2), (0, 4));
        // Past the end.
        assert_eq!(layout.visible(10_000.0, 100.0, 20.0, 0), (51, 52));
        assert_eq!(
            WrapLayout::build(&[], 10).visible(0.0, 100.0, 20.0, 3),
            (0, 0)
        );
    }

    #[test]
    fn large_files_stay_fast() {
        // About a 2 MB file: 50,000 lines of 40 characters.
        let rows = vec![RowChars::single(40); 50_000];
        let start = std::time::Instant::now();
        let layout = WrapLayout::build(&rows, 30);
        let _ = layout.visible(500_000.0, 900.0, 20.0, 20);
        let elapsed = start.elapsed();
        assert_eq!(layout.total(), 100_000);
        // Far below the 50 ms frame budget (debug build).
        assert!(elapsed.as_millis() < 25, "{elapsed:?}");
    }
}
