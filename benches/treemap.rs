//! Treemap layout benchmark.
//!
//! The layout lives in `crates/freespace-app/src/treemap/layout.rs`; this target
//! measures the real squarified pass on one million nodes laid out into a
//! 1600x900 viewport, the per-frame shape used by the canvas renderer.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use freespace_app::treemap::{Rect, squarify};

const NODES: u32 = 1_000_000;
const WIDTH: f32 = 1_600.0;
const HEIGHT: f32 = 900.0;

fn dataset(nodes: u32) -> Vec<(u32, f64)> {
    (0..nodes)
        .map(|index| (index, f64::from((index * 7_919) % 1_000_000 + 1)))
        .collect()
}

fn treemap_benchmarks(criterion: &mut Criterion) {
    let sizes = dataset(NODES);
    let bounds = Rect::new(0.0, 0.0, WIDTH, HEIGHT);

    criterion.bench_function("treemap_layout_1m", |bencher| {
        bencher.iter(|| black_box(squarify(black_box(&sizes), bounds)));
    });
}

criterion_group!(benches, treemap_benchmarks);
criterion_main!(benches);
