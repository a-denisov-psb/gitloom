//! Topological order + lane layout for 100 000 synthetic commits.
//!
//! `cargo bench -p git-graph`

// Benchmarks are test-only code.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use criterion::{criterion_group, criterion_main, Criterion};
use git_graph::{
    layout, layout_with, topo_order, ColorId, GraphCommit, Id, LaneHint, LaneHints, LayoutOptions,
    Wip,
};

/// Deterministic xorshift64.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x % n
    }
}

/// History with many parallel branches: parents are picked among the last
/// `spread` commits, ~20 % merges, occasional octopus merges and roots.
fn history(count: u32, spread: u64) -> Vec<GraphCommit> {
    let mut rng = Rng(0x5eed_1234);
    (0..count)
        .map(|i| {
            let parents = match rng.below(100) {
                _ if i == 0 => 0,
                0 => 0,
                1..=79 => 1,
                80..=97 => 2,
                _ => 3,
            };
            GraphCommit {
                id: Id(i),
                parents: (0..parents)
                    .map(|_| Id(i - 1 - rng.below(spread.min(u64::from(i))) as u32))
                    .collect(),
                time: i64::from(i) * 10 - rng.below(30) as i64,
            }
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let commits = history(100_000, 30);
    let order = topo_order(&commits).unwrap().order;
    let wip = Some(Wip {
        head: Some(order[0]),
    });
    let width = layout(&commits, &order, wip).unwrap().max_width;
    eprintln!("synthetic history: 100 000 commits, max width {width} lanes");

    let mut group = c.benchmark_group("100k commits");
    group.sample_size(20);
    group.bench_function("topo_order", |b| b.iter(|| topo_order(&commits).unwrap()));
    group.bench_function("layout", |b| {
        b.iter(|| layout(&commits, &order, wip).unwrap())
    });
    // Four groups plus unhinted commits, like a Git flow layout.
    let mut rng = Rng(0x41a7);
    let hints: Vec<Option<LaneHint>> = (0..commits.len())
        .map(|_| {
            let group = rng.below(5) as u16;
            (group < 4).then_some(LaneHint {
                group,
                color: ColorId(u32::from(group)),
            })
        })
        .collect();
    let options = LayoutOptions {
        wip,
        hints: Some(LaneHints { commits: &hints }),
        ..LayoutOptions::default()
    };
    group.bench_function("layout with 4 lane groups", |b| {
        b.iter(|| layout_with(&commits, &order, &options).unwrap())
    });
    group.bench_function("topo_order + layout", |b| {
        b.iter(|| {
            let order = topo_order(&commits).unwrap().order;
            layout(&commits, &order, wip).unwrap()
        })
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
