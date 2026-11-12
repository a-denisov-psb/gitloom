//! Widths of the graph table columns (M11-02, M11-16): every column has its
//! own width and handle; dragging changes only that column. The table is
//! as wide as its columns: wider than the viewport it scrolls
//! horizontally, narrower the commit message grows to fill it.
//! Date and SHA are not resizable (fix 2026-11-12): their content has a
//! constant width, so the view gives them a fixed width fitted to it; they
//! have no stored width and no handle.
//! Pure: no iced types; the view measures content and passes it in.

use serde::{Deserialize, Serialize};

/// Narrowest width of every resizable column except Graph and Message.
pub const MIN_WIDTH: f32 = 60.0;
/// Narrowest commit message column; also its "fill" setting.
pub const MIN_MESSAGE_WIDTH: f32 = 120.0;
/// The optimal Graph width takes at most this share of the viewport.
pub const MAX_GRAPH_SHARE: f32 = 0.5;

/// A column of the graph table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnId {
    Refs,
    Graph,
    Message,
    Author,
    Date,
    Sha,
}

impl ColumnId {
    pub const ALL: [ColumnId; 6] = [
        ColumnId::Refs,
        ColumnId::Graph,
        ColumnId::Message,
        ColumnId::Author,
        ColumnId::Date,
        ColumnId::Sha,
    ];

    /// The column has a handle and a stored width (Date and SHA do not).
    pub fn is_resizable(self) -> bool {
        !matches!(self, Self::Date | Self::Sha)
    }
}

/// Column widths in logical pixels, a global setting (`settings.json`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnWidths {
    pub refs: f32,
    /// `None`: as wide as the lanes of the loaded graph (2 … 12 lanes), the
    /// width before the first resize.
    pub graph: Option<f32>,
    /// Set width of the commit message; it grows to fill a wider viewport
    /// ([`MIN_MESSAGE_WIDTH`]: "fill").
    pub message: f32,
    pub author: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self {
            refs: 200.0,
            graph: None,
            message: MIN_MESSAGE_WIDTH,
            author: 130.0,
        }
    }
}

impl ColumnWidths {
    /// Set width of `column`; `auto_graph` is the Graph width while it was
    /// never resized. The message's effective width is [`Layout::message`].
    /// `None` for the fixed columns (Date, SHA): the view knows their width.
    pub fn width(&self, column: ColumnId, auto_graph: f32) -> Option<f32> {
        match column {
            ColumnId::Refs => Some(self.refs),
            ColumnId::Graph => Some(self.graph.unwrap_or(auto_graph)),
            ColumnId::Message => Some(self.message),
            ColumnId::Author => Some(self.author),
            ColumnId::Date | ColumnId::Sha => None,
        }
    }

    /// Sets the width of a resizable column; fixed columns are ignored.
    pub fn set(&mut self, column: ColumnId, width: f32) {
        match column {
            ColumnId::Refs => self.refs = width,
            ColumnId::Graph => self.graph = Some(width),
            ColumnId::Message => self.message = width,
            ColumnId::Author => self.author = width,
            ColumnId::Date | ColumnId::Sha => {}
        }
    }

    /// Widths from a settings file made usable: values that are not finite
    /// or below the minimum become the minimum (`min_graph` for Graph).
    pub fn sanitized(mut self, min_graph: f32) -> Self {
        for column in ColumnId::ALL {
            let min = min_width(column, min_graph);
            let fix = |w: f32| if w.is_finite() { w.max(min) } else { min };
            match column {
                ColumnId::Graph => self.graph = self.graph.map(fix),
                _ => {
                    if let Some(width) = self.width(column, 0.0) {
                        self.set(column, fix(width));
                    }
                }
            }
        }
        self
    }
}

/// Minimum width of `column`; `min_graph` is the width of one lane.
pub fn min_width(column: ColumnId, min_graph: f32) -> f32 {
    match column {
        ColumnId::Graph => min_graph,
        ColumnId::Message => MIN_MESSAGE_WIDTH,
        _ => MIN_WIDTH,
    }
}

/// Horizontal geometry of the table in a viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// Effective message width: its set width, or more to fill.
    pub message: f32,
    /// Width of the whole table (columns and `fixed`).
    pub table: f32,
    /// Largest horizontal scroll offset (0: everything fits).
    pub max_offset: f32,
}

/// The table in a viewport `viewport` wide: `fixed` (paddings and the
/// scrollbar room), `others` (all shown columns but the message summed)
/// and the message's set width.
pub fn layout(viewport: f32, fixed: f32, others: f32, message: f32) -> Layout {
    let remaining = viewport - fixed - others;
    let message = message.max(remaining).max(MIN_MESSAGE_WIDTH);
    let table = fixed + others + message;
    Layout {
        message,
        table,
        max_offset: (table - viewport).max(0.0),
    }
}

/// Width while dragging: the width at the start (`origin`) plus the cursor
/// movement, at least `min`. Only this column changes; the table grows or
/// shrinks with it.
pub fn dragged(origin: f32, delta: f32, min: f32) -> f32 {
    (origin + delta).max(min)
}

/// Optimal Graph width: all `lanes_width` of the loaded layout, but at most
/// [`MAX_GRAPH_SHARE`] of the viewport.
pub fn graph_optimal(lanes_width: f32, viewport: f32) -> f32 {
    lanes_width.min(viewport * MAX_GRAPH_SHARE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_former_fixed_widths() {
        let widths = ColumnWidths::default();
        assert_eq!(widths.width(ColumnId::Refs, 42.0), Some(200.0));
        assert_eq!(widths.width(ColumnId::Graph, 42.0), Some(42.0));
        assert_eq!(
            widths.width(ColumnId::Message, 42.0),
            Some(MIN_MESSAGE_WIDTH)
        );
        assert_eq!(widths.width(ColumnId::Author, 42.0), Some(130.0));
        // Date and SHA have a fixed width the view computes.
        for fixed in [ColumnId::Date, ColumnId::Sha] {
            assert_eq!(widths.width(fixed, 42.0), None);
            assert!(!fixed.is_resizable());
            let mut changed = widths;
            changed.set(fixed, 500.0);
            assert_eq!(changed, widths);
        }
    }

    #[test]
    fn old_settings_with_date_and_sha_widths_load() {
        let old: ColumnWidths =
            serde_json::from_str(r#"{"refs": 210, "author": 140, "date": 300, "sha": 90}"#)
                .unwrap();
        assert_eq!(old.refs, 210.0);
        assert_eq!(old.author, 140.0);
        assert!(!serde_json::to_string(&old).unwrap().contains("date"));
    }

    #[test]
    fn drag_changes_one_column_only() {
        let mut widths = ColumnWidths::default();
        let before = widths;
        widths.set(ColumnId::Author, dragged(130.0, 40.0, MIN_WIDTH));
        assert_eq!(widths.author, 170.0);
        // Neighbours keep their widths.
        assert_eq!(widths.message, before.message);
        assert_eq!(widths.refs, before.refs);
        // Minimums.
        assert_eq!(dragged(130.0, -500.0, MIN_WIDTH), MIN_WIDTH);
        assert_eq!(dragged(200.0, -500.0, MIN_MESSAGE_WIDTH), MIN_MESSAGE_WIDTH);
        assert_eq!(dragged(60.0, -100.0, 30.0), 30.0);
        // No upper limit: the table scrolls.
        assert_eq!(dragged(200.0, 900.0, MIN_WIDTH), 1100.0);
    }

    #[test]
    fn narrower_tables_let_the_message_fill() {
        // Viewport 1000, fixed 28, other columns 535: 437 left.
        let layout = layout(1000.0, 28.0, 535.0, MIN_MESSAGE_WIDTH);
        assert_eq!(layout.message, 437.0);
        assert_eq!(layout.table, 1000.0);
        assert_eq!(layout.max_offset, 0.0);
        // A set width above the remaining space wins and the table scrolls.
        let wide = layout_for(600.0);
        assert_eq!(wide.message, 600.0);
        assert_eq!(wide.table, 1163.0);
        assert_eq!(wide.max_offset, 163.0);
    }

    fn layout_for(message: f32) -> Layout {
        layout(1000.0, 28.0, 535.0, message)
    }

    #[test]
    fn wider_tables_scroll() {
        // Columns alone wider than the viewport: the message keeps its
        // minimum and the rest scrolls.
        let layout = layout(500.0, 28.0, 535.0, MIN_MESSAGE_WIDTH);
        assert_eq!(layout.message, MIN_MESSAGE_WIDTH);
        assert_eq!(layout.table, 683.0);
        assert_eq!(layout.max_offset, 183.0);
    }

    #[test]
    fn graph_optimal_is_capped_at_half_the_viewport() {
        assert_eq!(graph_optimal(120.0, 1000.0), 120.0);
        assert_eq!(graph_optimal(800.0, 1000.0), 500.0);
    }

    #[test]
    fn sanitized_repairs_broken_values() {
        let widths = ColumnWidths {
            refs: f32::NAN,
            graph: Some(5.0),
            message: 3.0,
            author: -3.0,
        }
        .sanitized(30.0);
        assert_eq!(widths.refs, MIN_WIDTH);
        assert_eq!(widths.graph, Some(30.0));
        assert_eq!(widths.message, MIN_MESSAGE_WIDTH);
        assert_eq!(widths.author, MIN_WIDTH);
        assert_eq!(ColumnWidths::default().sanitized(30.0).graph, None);
    }

    #[test]
    fn settings_round_trip() {
        let widths = ColumnWidths {
            graph: Some(84.0),
            message: 400.0,
            ..ColumnWidths::default()
        };
        let json = serde_json::to_string(&widths).unwrap();
        assert_eq!(serde_json::from_str::<ColumnWidths>(&json).unwrap(), widths);
        // Missing fields (e.g. a file from before M11-16) fall back.
        let partial: ColumnWidths = serde_json::from_str(r#"{"refs": 150}"#).unwrap();
        assert_eq!(partial.refs, 150.0);
        assert_eq!(partial.message, MIN_MESSAGE_WIDTH);
    }
}
