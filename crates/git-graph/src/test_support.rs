//! Helpers shared by the unit tests: history builder, random histories,
//! a compact text rendering of layouts and an invariant checker.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::{ColorId, GraphCommit, Id, Layout, Row, Segment, Wip};

/// Builds the input from `(parents, time)` pairs; ids are positions.
pub fn history(spec: &[(&[u32], i64)]) -> Vec<GraphCommit> {
    spec.iter()
        .enumerate()
        .map(|(i, (parents, time))| GraphCommit {
            id: Id(i as u32),
            parents: parents.iter().map(|&p| Id(p)).collect(),
            time: *time,
        })
        .collect()
}

/// Deterministic xorshift64 generator; no external dependency needed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform-ish value in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Random history with `count` commits: mostly linear segments, forks,
/// merges (including octopus), several roots, clock skew and, if `missing`
/// is set, parents missing from the input.
/// `spread` bounds how far back parents are picked, i.e. how many branches run in parallel.
pub fn random_history(rng: &mut Rng, count: u32, spread: u64, missing: bool) -> Vec<GraphCommit> {
    (0..count)
        .map(|i| {
            let parent_count = match rng.below(100) {
                _ if i == 0 => 0,
                0..=2 => 0,
                3..=79 => 1,
                80..=96 => 2,
                _ => 3,
            };
            let mut parents = Vec::new();
            for _ in 0..parent_count {
                let parent = if missing && rng.below(50) == 0 {
                    // Missing from the input.
                    count + rng.below(5) as u32
                } else {
                    i - 1 - rng.below(spread.min(u64::from(i))) as u32
                };
                parents.push(Id(parent));
            }
            GraphCommit {
                id: Id(i),
                parents,
                // Mostly increasing with occasional skew.
                time: i64::from(i) * 10 - rng.below(30) as i64,
            }
        })
        .collect()
}

/// Renders one line per row: lane cells, node color, then the curve endpoints.
///
/// Cells: `*` node, `o` WIP node, `|` through line, `:` dashed through line.
/// Tokens: `c<n>` node color, `^<lane>` line into the node from the top of
/// `lane`, `v<lane>` line from the node to the bottom of `lane`; suffix `.`
/// marks a dashed line, `c<n>` a color different from the node color.
pub fn render(layout: &Layout, has_wip: bool) -> String {
    let cell_width = usize::from(layout.max_width) * 2;
    layout
        .rows()
        .enumerate()
        .map(|(i, row)| render_row(&row, has_wip && i == 0, cell_width))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_row(row: &Row, is_wip: bool, cell_width: usize) -> String {
    let mut cells = vec![' '; usize::from(row.width)];
    for segment in &row.segments {
        if let Segment::Through { lane, dashed, .. } = *segment {
            cells[usize::from(lane)] = if dashed { ':' } else { '|' };
        }
    }
    cells[usize::from(row.lane)] = if is_wip { 'o' } else { '*' };
    let cells: String = cells.iter().map(|c| format!("{c} ")).collect();

    let mut out = format!("{cells:<cell_width$}c{}", row.color.0);
    for segment in &row.segments {
        let (marker, lane, color, dashed) = match *segment {
            Segment::Through { .. } => continue,
            Segment::IntoNode {
                from,
                color,
                dashed,
            } => ('^', from, color, dashed),
            Segment::FromNode { to, color, dashed } => ('v', to, color, dashed),
        };
        let _ = write!(out, " {marker}{lane}");
        if dashed {
            out.push('.');
        }
        if color != row.color {
            let _ = write!(out, "c{}", color.0);
        }
    }
    out.trim_end().to_owned()
}

/// Edge as `(child row, parent row or None for an open end, dashed)`.
type Edge = (usize, Option<usize>, bool);

struct LaneState {
    color: ColorId,
    dashed: bool,
    /// Child rows (and dashed flags) of all edges running in this lane.
    sources: Vec<(usize, bool)>,
}

/// Checks the layout invariants and that every parent edge of the input
/// appears exactly once as a connection in the output.
pub fn verify(commits: &[GraphCommit], order: &[Id], wip: Option<Wip>, layout: &Layout) {
    verify_with(commits, order, wip, layout, true);
}

/// Like [`verify`] for layouts with lane hints: a node may sit in any
/// incoming lane or, if none belongs to its group, in a free lane.
pub fn verify_hinted(commits: &[GraphCommit], order: &[Id], layout: &Layout) {
    verify_with(commits, order, None, layout, false);
}

fn verify_with(
    commits: &[GraphCommit],
    order: &[Id],
    wip: Option<Wip>,
    layout: &Layout,
    lowest_incoming: bool,
) {
    let offset = usize::from(wip.is_some());
    assert_eq!(layout.len(), order.len() + offset);
    let rows: Vec<Row> = layout.rows().collect();

    let mut row_of = vec![usize::MAX; commits.len()];
    for (i, id) in order.iter().enumerate() {
        row_of[id.index()] = i + offset;
    }
    let target = |parent: Id| row_of.get(parent.index()).copied();

    let mut expected: Vec<Edge> = Vec::new();
    if let Some(head) = wip.and_then(|w| w.head) {
        expected.push((0, target(head), true));
    }
    for (i, id) in order.iter().enumerate() {
        for &parent in &commits[id.index()].parents {
            expected.push((i + offset, target(parent), false));
        }
    }

    let mut lanes: BTreeMap<u16, LaneState> = BTreeMap::new();
    let mut actual: Vec<Edge> = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        // Upper half must continue exactly the lanes left open by the rows above.
        let mut upper = Vec::new();
        let mut into = Vec::new();
        for segment in &row.segments {
            let (lane, color, dashed) = match *segment {
                Segment::Through {
                    lane,
                    color,
                    dashed,
                } => (lane, color, dashed),
                Segment::IntoNode {
                    from,
                    color,
                    dashed,
                } => {
                    into.push(from);
                    (from, color, dashed)
                }
                Segment::FromNode { .. } => continue,
            };
            upper.push(lane);
            let state = lanes
                .get(&lane)
                .unwrap_or_else(|| panic!("row {r}: segment in free lane {lane}"));
            assert_eq!(state.color, color, "row {r}: color of lane {lane}");
            assert_eq!(state.dashed, dashed, "row {r}: dash of lane {lane}");
        }
        assert_eq!(
            upper,
            lanes.keys().copied().collect::<Vec<_>>(),
            "row {r}: upper half does not continue the open lanes"
        );

        match into.iter().min() {
            Some(_) if !lowest_incoming => assert!(
                into.contains(&row.lane) || !lanes.contains_key(&row.lane),
                "row {r}: node on a lane that passes by"
            ),
            Some(&lowest) => assert_eq!(
                row.lane, lowest,
                "row {r}: node not in lowest incoming lane"
            ),
            None => assert!(
                !lanes.contains_key(&row.lane),
                "row {r}: tip on occupied lane"
            ),
        }
        for from in into {
            let state = lanes.remove(&from).unwrap();
            actual.extend(state.sources.into_iter().map(|(src, d)| (src, Some(r), d)));
        }

        // Lower half: first parent leaves from the node lane in the node color.
        let mut first = true;
        for segment in &row.segments {
            let Segment::FromNode { to, color, dashed } = *segment else {
                continue;
            };
            if first {
                assert_eq!((to, color), (row.lane, row.color), "row {r}: first parent");
                first = false;
            }
            match lanes.get_mut(&to) {
                Some(state) => {
                    assert_eq!(state.color, color, "row {r}: color of joined lane {to}");
                    state.sources.push((r, dashed));
                    state.dashed &= dashed;
                }
                None => {
                    lanes.insert(
                        to,
                        LaneState {
                            color,
                            dashed,
                            sources: vec![(r, dashed)],
                        },
                    );
                }
            }
        }

        let highest = row
            .segments
            .iter()
            .map(|s| s.lane())
            .fold(row.lane, u16::max);
        assert_eq!(row.width, highest + 1, "row {r}: width");
    }
    for state in lanes.into_values() {
        actual.extend(state.sources.into_iter().map(|(src, d)| (src, None, d)));
    }

    expected.sort_unstable();
    actual.sort_unstable();
    assert_eq!(actual, expected, "edges differ");
    assert_eq!(
        layout.max_width,
        rows.iter().map(|r| r.width).max().unwrap_or(0)
    );
}
