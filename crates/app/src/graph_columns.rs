//! Widths of the graph table columns (M11-02): dragging, minimum widths,
//! optimal width and the commit message column that fills the rest.
//! Pure: no iced types; the view measures content and passes it in.

use serde::{Deserialize, Serialize};

/// Narrowest width of every resizable column except Graph.
pub const MIN_WIDTH: f32 = 60.0;
/// The commit message column never gets narrower than this by dragging.
pub const MIN_MESSAGE_WIDTH: f32 = 120.0;
/// The optimal Graph width takes at most this share of the table.
pub const MAX_GRAPH_SHARE: f32 = 0.5;

/// A resizable column. The commit message column fills the rest and has no
/// width of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnId {
    Refs,
    Graph,
    Author,
    Date,
    Sha,
}

impl ColumnId {
    pub const ALL: [ColumnId; 5] = [
        ColumnId::Refs,
        ColumnId::Graph,
        ColumnId::Author,
        ColumnId::Date,
        ColumnId::Sha,
    ];
}

/// Column widths in logical pixels, a global setting (`settings.json`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnWidths {
    pub refs: f32,
    /// `None`: as wide as the lanes of the loaded graph (2 … 12 lanes), the
    /// width before the first resize.
    pub graph: Option<f32>,
    pub author: f32,
    pub date: f32,
    pub sha: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self {
            refs: 200.0,
            graph: None,
            author: 130.0,
            date: 135.0,
            sha: 70.0,
        }
    }
}

impl ColumnWidths {
    /// Width of `column`; `auto_graph` is the Graph width while it was never
    /// resized.
    pub fn width(&self, column: ColumnId, auto_graph: f32) -> f32 {
        match column {
            ColumnId::Refs => self.refs,
            ColumnId::Graph => self.graph.unwrap_or(auto_graph),
            ColumnId::Author => self.author,
            ColumnId::Date => self.date,
            ColumnId::Sha => self.sha,
        }
    }

    pub fn set(&mut self, column: ColumnId, width: f32) {
        match column {
            ColumnId::Refs => self.refs = width,
            ColumnId::Graph => self.graph = Some(width),
            ColumnId::Author => self.author = width,
            ColumnId::Date => self.date = width,
            ColumnId::Sha => self.sha = width,
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
                    let width = fix(self.width(column, 0.0));
                    self.set(column, width);
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
        _ => MIN_WIDTH,
    }
}

/// Width of the commit message column: what `table` leaves after `fixed`
/// (paddings, scrollbar) and the shown columns (`widths` of them summed).
pub fn message_width(table: f32, fixed: f32, columns: f32) -> f32 {
    table - fixed - columns
}

/// How much wider a column may get before the message column drops below
/// [`MIN_MESSAGE_WIDTH`].
pub fn room(message_width: f32) -> f32 {
    (message_width - MIN_MESSAGE_WIDTH).max(0.0)
}

/// Width while dragging: the width at the start (`origin`) plus the cursor
/// movement, at least `min`, and at most `origin + room` (never below
/// `origin` itself, so a table that is already too narrow does not snap).
pub fn dragged(origin: f32, delta: f32, room: f32, min: f32) -> f32 {
    let max = (origin + room).max(origin).max(min);
    (origin + delta).clamp(min, max)
}

/// Width after a double click on the border: `optimal`, within the same
/// limits as a drag.
pub fn fitted(origin: f32, optimal: f32, room: f32, min: f32) -> f32 {
    dragged(origin, optimal - origin, room, min)
}

/// Optimal Graph width: all `lanes_width` of the loaded layout, but at most
/// [`MAX_GRAPH_SHARE`] of the table.
pub fn graph_optimal(lanes_width: f32, table: f32) -> f32 {
    lanes_width.min(table * MAX_GRAPH_SHARE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_former_fixed_widths() {
        let widths = ColumnWidths::default();
        assert_eq!(widths.width(ColumnId::Refs, 42.0), 200.0);
        assert_eq!(widths.width(ColumnId::Graph, 42.0), 42.0);
        assert_eq!(widths.width(ColumnId::Author, 42.0), 130.0);
        assert_eq!(widths.width(ColumnId::Date, 42.0), 135.0);
        assert_eq!(widths.width(ColumnId::Sha, 42.0), 70.0);
    }

    #[test]
    fn set_fixes_the_graph_width() {
        let mut widths = ColumnWidths::default();
        widths.set(ColumnId::Graph, 90.0);
        assert_eq!(widths.width(ColumnId::Graph, 42.0), 90.0);
        widths.set(ColumnId::Sha, 80.0);
        assert_eq!(widths.sha, 80.0);
    }

    #[test]
    fn drag_follows_the_cursor_within_limits() {
        // Wider and narrower.
        assert_eq!(dragged(200.0, 30.0, 100.0, MIN_WIDTH), 230.0);
        assert_eq!(dragged(200.0, -50.0, 100.0, MIN_WIDTH), 150.0);
        // Minimum width.
        assert_eq!(dragged(200.0, -500.0, 100.0, MIN_WIDTH), MIN_WIDTH);
        assert_eq!(dragged(60.0, -10.0, 100.0, 30.0), 50.0);
        assert_eq!(dragged(60.0, -100.0, 100.0, 30.0), 30.0);
        // The message column keeps its minimum.
        assert_eq!(dragged(200.0, 500.0, 100.0, MIN_WIDTH), 300.0);
        // No room left: the column cannot grow, but can shrink.
        assert_eq!(dragged(200.0, 20.0, 0.0, MIN_WIDTH), 200.0);
        assert_eq!(dragged(200.0, -20.0, 0.0, MIN_WIDTH), 180.0);
    }

    #[test]
    fn message_fills_the_rest() {
        let message = message_width(1000.0, 28.0, 200.0 + 60.0 + 130.0 + 135.0 + 70.0);
        assert_eq!(message, 377.0);
        assert_eq!(room(message), 257.0);
        assert_eq!(room(100.0), 0.0);
    }

    #[test]
    fn fit_uses_the_optimal_width_within_limits() {
        assert_eq!(fitted(200.0, 140.0, 100.0, MIN_WIDTH), 140.0);
        assert_eq!(fitted(200.0, 280.0, 100.0, MIN_WIDTH), 280.0);
        assert_eq!(fitted(200.0, 400.0, 100.0, MIN_WIDTH), 300.0);
        assert_eq!(fitted(200.0, 10.0, 100.0, MIN_WIDTH), MIN_WIDTH);
    }

    #[test]
    fn graph_optimal_is_capped_at_half_the_table() {
        assert_eq!(graph_optimal(120.0, 1000.0), 120.0);
        assert_eq!(graph_optimal(800.0, 1000.0), 500.0);
    }

    #[test]
    fn sanitized_repairs_broken_values() {
        let widths = ColumnWidths {
            refs: f32::NAN,
            graph: Some(5.0),
            author: -3.0,
            date: 300.0,
            sha: f32::INFINITY,
        }
        .sanitized(30.0);
        assert_eq!(widths.refs, MIN_WIDTH);
        assert_eq!(widths.graph, Some(30.0));
        assert_eq!(widths.author, MIN_WIDTH);
        assert_eq!(widths.date, 300.0);
        assert_eq!(widths.sha, MIN_WIDTH);
        assert_eq!(ColumnWidths::default().sanitized(30.0).graph, None);
    }

    #[test]
    fn settings_round_trip() {
        let widths = ColumnWidths {
            graph: Some(84.0),
            ..ColumnWidths::default()
        };
        let json = serde_json::to_string(&widths).unwrap();
        assert_eq!(serde_json::from_str::<ColumnWidths>(&json).unwrap(), widths);
        // Missing fields fall back to the defaults.
        let partial: ColumnWidths = serde_json::from_str(r#"{"refs": 150}"#).unwrap();
        assert_eq!(partial.refs, 150.0);
        assert_eq!(partial.author, 130.0);
    }
}
