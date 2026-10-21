//! Lane layout: per-row graph geometry (see `docs/ARCHITECTURE.md` → "Graph layout").
//!
//! Wide histories keep hundreds of lanes open, so storing a `Through`
//! segment per lane and row would grow with rows × lanes. The layout
//! therefore stores vertical runs ([`Span`]) and the curve segments per row,
//! and rebuilds full [`Row`]s on demand.

use crate::{check_ids, Error, GraphCommit, Id};

/// Color of a lane. Assigned sequentially; the UI maps it onto its palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorId(pub u32);

/// Virtual "work in progress" row drawn above all commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wip {
    /// Commit HEAD points to; `None` in a repository without commits.
    pub head: Option<Id>,
}

/// Rows between two checkpoints of the span index.
const BUCKET: usize = 64;

/// Geometry of all rows: the WIP row first (if requested), then one row per
/// commit in the given order. Use [`Layout::row`] to get a row's segments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    /// Node lane, node color and width per row.
    nodes: Vec<(u16, ColorId, u16)>,
    /// `IntoNode` and `FromNode` segments of all rows, row by row.
    curves: Vec<Segment>,
    /// `curves[curve_start[r]..curve_start[r + 1]]` belong to row `r`.
    curve_start: Vec<u32>,
    /// Vertical runs of `Through` segments, sorted by first row.
    spans: Vec<Span>,
    /// Spans covering row `b * BUCKET`, for every bucket `b`.
    checkpoints: Vec<Vec<u32>>,
    /// Maximum [`Row::width`] over all rows.
    pub max_width: u16,
}

/// `Through` segments of one lane in rows `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: u32,
    end: u32,
    lane: u16,
    color: ColorId,
    dashed: bool,
}

impl Layout {
    /// Number of rows.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Lane of the node in row `index` (cheaper than building the row).
    pub fn node_lane(&self, index: usize) -> Option<u16> {
        self.nodes.get(index).map(|n| n.0)
    }

    /// Geometry of row `index`.
    pub fn row(&self, index: usize) -> Option<Row> {
        let &(lane, color, width) = self.nodes.get(index)?;
        let curves =
            &self.curves[self.curve_start[index] as usize..self.curve_start[index + 1] as usize];

        let mut segments: Vec<Segment> = self
            .spans_at(index)
            .map(|span| Segment::Through {
                lane: span.lane,
                color: span.color,
                dashed: span.dashed,
            })
            .chain(
                curves
                    .iter()
                    .copied()
                    .filter(|s| matches!(s, Segment::IntoNode { .. })),
            )
            .collect();
        // Every lane appears at most once in the upper half.
        segments.sort_unstable_by_key(|s| s.lane());
        segments.extend(
            curves
                .iter()
                .copied()
                .filter(|s| matches!(s, Segment::FromNode { .. })),
        );
        Some(Row {
            lane,
            color,
            segments,
            width,
        })
    }

    /// All rows in order (built on demand).
    pub fn rows(&self) -> impl Iterator<Item = Row> + '_ {
        (0..self.len()).filter_map(|r| self.row(r))
    }

    /// Spans with `start <= row < end`.
    fn spans_at(&self, row: usize) -> impl Iterator<Item = &Span> {
        let bucket = row / BUCKET;
        let base = (bucket * BUCKET) as u32;
        let row = row as u32;
        let from_checkpoint = self
            .checkpoints
            .get(bucket)
            .into_iter()
            .flatten()
            .filter_map(|&i| self.spans.get(i as usize));
        // Spans starting after the checkpoint row, up to `row`.
        let lo = self.spans.partition_point(|s| s.start <= base);
        let hi = self.spans.partition_point(|s| s.start <= row);
        from_checkpoint
            .chain(self.spans[lo..hi.max(lo)].iter())
            .filter(move |s| s.start <= row && row < s.end)
    }
}

/// Geometry of one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Lane of the node.
    pub lane: u16,
    /// Color of the node.
    pub color: ColorId,
    /// Upper-half segments (`Through`, `IntoNode`) in lane order, then
    /// lower-half segments (`FromNode`) in parent order.
    pub segments: Vec<Segment>,
    /// Number of lanes this row touches (highest used lane + 1).
    pub width: u16,
}

/// A line inside one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    /// Vertical line over the full row height.
    Through {
        lane: u16,
        color: ColorId,
        dashed: bool,
    },
    /// From the top of lane `from` to the node.
    IntoNode {
        from: u16,
        color: ColorId,
        dashed: bool,
    },
    /// From the node to the bottom of lane `to`.
    FromNode {
        to: u16,
        color: ColorId,
        dashed: bool,
    },
}

impl Segment {
    /// The lane of this segment away from the node: its own lane for
    /// `Through`, the start lane for `IntoNode`, the end lane for `FromNode`.
    pub fn lane(self) -> u16 {
        match self {
            Self::Through { lane, .. } => lane,
            Self::IntoNode { from, .. } => from,
            Self::FromNode { to, .. } => to,
        }
    }
}

/// Id of the WIP row. Never a real commit: [`check_ids`] limits ids to `u32::MAX - 1`,
/// and no lane can wait for it because the WIP row comes first.
const WIP_ID: Id = Id(u32::MAX);

/// Computes the lane layout for `commits` shown in `order`.
///
/// `order` must contain every commit exactly once and should be topological
/// (as returned by [`crate::topo_order`]); edges to parents shown above their
/// child stay open to the bottom.
pub fn layout(commits: &[GraphCommit], order: &[Id], wip: Option<Wip>) -> Result<Layout, Error> {
    check_ids(commits)?;
    if order.len() != commits.len() {
        return Err(Error::InvalidOrder);
    }
    let mut seen = vec![false; commits.len()];
    for id in order {
        match seen.get_mut(id.index()) {
            Some(seen) if !*seen => *seen = true,
            _ => return Err(Error::InvalidOrder),
        }
    }

    let rows = order.len() + usize::from(wip.is_some());
    if u32::try_from(rows).is_err() {
        return Err(Error::TooManyCommits(rows));
    }
    let mut lanes = Lanes::default();
    let mut out = Layout {
        nodes: Vec::with_capacity(rows),
        curve_start: Vec::with_capacity(rows + 1),
        ..Layout::default()
    };
    out.curve_start.push(0);
    if let Some(wip) = wip {
        let head: Vec<Id> = wip.head.into_iter().collect();
        lanes.row(&mut out, WIP_ID, &head, true)?;
    }
    for id in order {
        lanes.row(&mut out, *id, &commits[id.index()].parents, false)?;
    }
    // Lanes still open run to the bottom.
    let end = out.nodes.len() as u32;
    for (lane, slot) in lanes.slots.iter().enumerate() {
        if let Some(slot) = slot {
            lanes.spans.push(slot.span(lane_u16(lane)?, end));
        }
    }

    let mut spans = std::mem::take(&mut lanes.spans);
    spans.retain(|s| s.start < s.end);
    spans.sort_unstable_by_key(|s| (s.start, s.lane));
    out.checkpoints = vec![Vec::new(); rows.div_ceil(BUCKET)];
    for (i, span) in spans.iter().enumerate() {
        let first = (span.start as usize).div_ceil(BUCKET);
        let last = (span.end as usize - 1) / BUCKET;
        for bucket in first..=last {
            if let Some(list) = out.checkpoints.get_mut(bucket) {
                list.push(i as u32);
            }
        }
    }
    out.spans = spans;
    out.max_width = out.nodes.iter().map(|n| n.2).max().unwrap_or(0);
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    waiting_for: Id,
    color: ColorId,
    dashed: bool,
    /// First row in which this lane is drawn as `Through`.
    since: u32,
}

impl Slot {
    fn span(&self, lane: u16, end: u32) -> Span {
        Span {
            start: self.since,
            end,
            lane,
            color: self.color,
            dashed: self.dashed,
        }
    }
}

#[derive(Debug, Default)]
struct Lanes {
    slots: Vec<Option<Slot>>,
    next_color: u32,
    /// Finished `Through` runs.
    spans: Vec<Span>,
}

impl Lanes {
    /// Lays out the row of commit `id` into `out` and updates the lane state.
    /// `dashed` marks the edges to `parents` (WIP row only).
    fn row(&mut self, out: &mut Layout, id: Id, parents: &[Id], dashed: bool) -> Result<(), Error> {
        let row = out.nodes.len() as u32;
        // Slots are trimmed after each row, so the last one is occupied.
        let mut highest = self.slots.len().checked_sub(1);

        // Upper half: lanes waiting for this commit end in the node; all
        // others pass by (recorded as spans).
        let mut node = None;
        for (lane, slot) in self.slots.iter_mut().enumerate() {
            let Some(waiting) = slot else { continue };
            if waiting.waiting_for != id {
                continue;
            }
            node.get_or_insert((lane, waiting.color));
            out.curves.push(Segment::IntoNode {
                from: lane_u16(lane)?,
                color: waiting.color,
                dashed: waiting.dashed,
            });
            self.spans.push(waiting.span(lane_u16(lane)?, row));
            *slot = None;
        }

        // Branch tips start in the lowest free lane with a new color.
        let (lane, color) = match node {
            Some(node) => node,
            None => (self.free_lane(), self.new_color()),
        };

        // Lower half: the first parent continues in the node lane, the others
        // join a lane already waiting for them or open a new one.
        let mut parents = parents.iter().copied();
        if let Some(first) = parents.next() {
            self.set(
                lane,
                Slot {
                    waiting_for: first,
                    color,
                    dashed,
                    since: row + 1,
                },
            );
            out.curves.push(Segment::FromNode {
                to: lane_u16(lane)?,
                color,
                dashed,
            });
        }
        for parent in parents {
            let waiting = self
                .slots
                .iter_mut()
                .enumerate()
                .find_map(|(lane, slot)| match slot {
                    Some(slot) if slot.waiting_for == parent => Some((lane, slot)),
                    _ => None,
                });
            let (to, to_color) = match waiting {
                Some((to, slot)) => {
                    // A solid edge sharing the lane makes it solid from the next row on.
                    if slot.dashed && !dashed && slot.since <= row {
                        self.spans.push(slot.span(lane_u16(to)?, row + 1));
                        slot.since = row + 1;
                    }
                    slot.dashed &= dashed;
                    (to, slot.color)
                }
                None => {
                    let (to, to_color) = (self.free_lane(), self.new_color());
                    self.set(
                        to,
                        Slot {
                            waiting_for: parent,
                            color: to_color,
                            dashed,
                            since: row + 1,
                        },
                    );
                    (to, to_color)
                }
            };
            highest = highest.max(Some(to));
            out.curves.push(Segment::FromNode {
                to: lane_u16(to)?,
                color: to_color,
                dashed,
            });
        }

        while matches!(self.slots.last(), Some(None)) {
            self.slots.pop();
        }

        let width = highest.map_or(lane, |h| h.max(lane)) + 1;
        let width = u16::try_from(width).map_err(|_| Error::TooManyLanes)?;
        out.nodes.push((lane_u16(lane)?, color, width));
        out.curve_start.push(out.curves.len() as u32);
        Ok(())
    }

    fn free_lane(&self) -> usize {
        self.slots
            .iter()
            .position(Option::is_none)
            .unwrap_or(self.slots.len())
    }

    /// Occupies `lane`, which is at most one past the last slot.
    fn set(&mut self, lane: usize, slot: Slot) {
        match self.slots.get_mut(lane) {
            Some(existing) => *existing = Some(slot),
            None => self.slots.push(Some(slot)),
        }
    }

    fn new_color(&mut self) -> ColorId {
        let color = ColorId(self.next_color);
        self.next_color = self.next_color.wrapping_add(1);
        color
    }
}

fn lane_u16(lane: usize) -> Result<u16, Error> {
    u16::try_from(lane).map_err(|_| Error::TooManyLanes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{history, random_history, render, verify, Rng};
    use crate::topo_order;

    /// Lays out `spec`, checks all invariants and renders the result.
    fn snapshot(spec: &[(&[u32], i64)], wip: Option<Wip>) -> String {
        let commits = history(spec);
        let order = topo_order(&commits).unwrap().order;
        let layout = layout(&commits, &order, wip).unwrap();
        verify(&commits, &order, wip, &layout);
        render(&layout, wip.is_some())
    }

    fn wip(head: u32) -> Option<Wip> {
        Some(Wip {
            head: Some(Id(head)),
        })
    }

    fn assert_snapshot(actual: String, expected: &[&str]) {
        assert_eq!(actual, expected.join("\n"));
    }

    #[test]
    fn empty() {
        assert_eq!(snapshot(&[], None), "");
    }

    #[test]
    fn linear() {
        assert_snapshot(
            snapshot(&[(&[1], 3), (&[2], 2), (&[], 1)], None),
            &["* c0 v0", "* c0 ^0 v0", "* c0 ^0"],
        );
    }

    #[test]
    fn fork() {
        assert_snapshot(
            snapshot(&[(&[], 1), (&[0], 2), (&[0], 3)], None),
            &["*   c0 v0", "| * c1 v1", "*   c0 ^0 ^1c1"],
        );
    }

    #[test]
    fn merge() {
        assert_snapshot(
            snapshot(&[(&[], 1), (&[0], 2), (&[0], 3), (&[1, 2], 4)], None),
            &[
                "*   c0 v0 v1c1",
                "| * c1 ^1 v1",
                "* | c0 ^0 v0",
                "*   c0 ^0 ^1c1",
            ],
        );
    }

    #[test]
    fn octopus_merge() {
        assert_snapshot(
            snapshot(
                &[(&[], 1), (&[0], 2), (&[0], 4), (&[0], 3), (&[1, 2, 3], 5)],
                None,
            ),
            &[
                "*     c0 v0 v1c1 v2c2",
                "| * | c1 ^1 v1",
                "| | * c2 ^2 v2",
                "* | | c0 ^0 v0",
                "*     c0 ^0 ^1c1 ^2c2",
            ],
        );
    }

    #[test]
    fn criss_cross_merge() {
        // The second parent of the second merge joins the lane already waiting for it.
        assert_snapshot(
            snapshot(
                &[(&[], 1), (&[0], 2), (&[0], 3), (&[1, 2], 4), (&[2, 1], 5)],
                None,
            ),
            &[
                "*     c0 v0 v1c1",
                "| | * c2 v2 v0c0",
                "* | | c0 ^0 v0",
                "| *   c1 ^1 ^2c2 v1",
                "*     c0 ^0 ^1c1",
            ],
        );
    }

    #[test]
    fn clock_skew_reuses_freed_lane() {
        assert_snapshot(
            snapshot(&[(&[], 100), (&[0], 50), (&[], 75)], None),
            &["* c0", "* c1 v0", "* c1 ^0"],
        );
    }

    #[test]
    fn multiple_roots_keep_lanes_stable() {
        assert_snapshot(
            snapshot(&[(&[], 1), (&[0], 3), (&[], 2), (&[2], 4)], None),
            &["*   c0 v0", "| * c1 v1", "* | c0 ^0", "  * c1 ^1"],
        );
    }

    #[test]
    fn missing_parents_stay_open() {
        assert_snapshot(
            snapshot(&[(&[5], 2), (&[0, 7], 3)], None),
            &["*   c0 v0 v1c1", "* | c0 ^0 v0"],
        );
    }

    #[test]
    fn tips_reuse_lowest_free_lane() {
        // Lane 1 is freed by a root while lane 2 is still open; the next tip takes lane 1.
        assert_snapshot(
            snapshot(
                &[
                    (&[], 1),
                    (&[0], 90),
                    (&[5], 85),
                    (&[0], 80),
                    (&[0], 70),
                    (&[], 75),
                ],
                None,
            ),
            &[
                "*     c0 v0",
                "| *   c1 v1",
                "| | * c2 v2",
                "| * | c1 ^1",
                "| * | c3 v1",
                "*     c0 ^0 ^1c3 ^2c2",
            ],
        );
    }

    #[test]
    fn wip_above_head() {
        assert_snapshot(
            snapshot(&[(&[1], 3), (&[2], 2), (&[], 1)], wip(0)),
            &["o c0 v0.", "* c0 ^0. v0", "* c0 ^0 v0", "* c0 ^0"],
        );
    }

    #[test]
    fn wip_edge_passes_newer_commits_dashed() {
        assert_snapshot(
            snapshot(&[(&[], 1), (&[0], 2), (&[0], 3)], wip(1)),
            &["o   c0 v0.", ": * c1 v1", "* | c0 ^0. v0", "*   c0 ^0 ^1c1"],
        );
    }

    #[test]
    fn wip_without_commits() {
        assert_snapshot(snapshot(&[], Some(Wip { head: None })), &["o c0"]);
    }

    #[test]
    fn wip_with_missing_head_stays_open() {
        assert_snapshot(snapshot(&[(&[], 1)], wip(9)), &["o   c0 v0.", ": * c1"]);
    }

    #[test]
    fn wip_on_head_with_children() {
        assert_snapshot(
            snapshot(&[(&[], 1), (&[0], 2)], wip(0)),
            &["o   c0 v0.", ": * c1 v1", "*   c0 ^0. ^1c1"],
        );
    }

    #[test]
    fn rejects_invalid_order() {
        let commits = history(&[(&[], 1), (&[0], 2)]);
        for order in [vec![Id(1)], vec![Id(1), Id(1)], vec![Id(1), Id(2)]] {
            assert_eq!(layout(&commits, &order, None), Err(Error::InvalidOrder));
        }
    }

    #[test]
    fn rejects_id_not_matching_position() {
        let mut commits = history(&[(&[], 1)]);
        commits[0].id = Id(3);
        assert_eq!(
            layout(&commits, &[Id(0)], None),
            Err(Error::IdMismatch {
                position: 0,
                id: Id(3)
            })
        );
    }

    #[test]
    fn random_histories_keep_every_edge() {
        let mut rng = Rng::new(0x5eed);
        for round in 0..1000 {
            let count = 1 + rng.below(60) as u32;
            let spread = 1 + rng.below(12);
            let commits = random_history(&mut rng, count, spread, true);
            let order = topo_order(&commits).unwrap().order;
            let wip = match round % 4 {
                0 => None,
                1 => Some(Wip { head: None }),
                2 => Some(Wip {
                    head: Some(Id(count + 7)),
                }),
                _ => Some(Wip {
                    head: Some(Id(rng.below(u64::from(count)) as u32)),
                }),
            };
            let layout = layout(&commits, &order, wip).unwrap();
            verify(&commits, &order, wip, &layout);
        }
    }

    #[test]
    fn long_wide_histories_keep_every_edge() {
        // Spans the span index over many buckets and with dozens of open lanes.
        let mut rng = Rng::new(0xbead);
        for round in 0..12 {
            let count = 500 + rng.below(1500) as u32;
            let spread = 5 + rng.below(60);
            let commits = random_history(&mut rng, count, spread, round % 2 == 0);
            let order = topo_order(&commits).unwrap().order;
            let wip = Some(Wip {
                head: Some(order[rng.below(u64::from(count)) as usize]),
            });
            let layout = layout(&commits, &order, wip).unwrap();
            verify(&commits, &order, wip, &layout);
        }
    }

    /// Quick timing check; the real benchmark follows in M1-15.
    /// Run with `cargo test -p git-graph --release -- --ignored --nocapture`.
    #[test]
    #[ignore = "timing check, run manually in release mode"]
    fn timing_100k_commits() {
        let commits = random_history(&mut Rng::new(42), 100_000, 30, false);
        let start = std::time::Instant::now();
        let order = topo_order(&commits).unwrap().order;
        let topo = start.elapsed();
        let layout = layout(
            &commits,
            &order,
            Some(Wip {
                head: Some(order[0]),
            }),
        )
        .unwrap();
        let total = start.elapsed();
        eprintln!(
            "100k commits: topo {topo:?}, topo + layout {total:?}, max width {}",
            layout.max_width
        );
    }
}
