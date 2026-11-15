//! Treemap layout benchmark.
//!
//! The interactive layout lives in `apps/freespace-gui/src/treemap`; a
//! benchmark target cannot import a binary crate, so this benchmark measures
//! the same per-frame work on the same input shape: ordering one million nodes
//! and grouping them into squarified rows.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

const NODES: usize = 1_000_000;
const BOUNDS: f64 = 1_600.0 * 900.0;

fn dataset(nodes: usize) -> Vec<u64> {
    (0..nodes)
        .map(|index| ((index * 7_919) % 1_000_000) as u64 + 1)
        .collect()
}

fn prepare_input(sizes: &[u64]) -> (Vec<f64>, usize) {
    let mut ordered: Vec<u64> = sizes.to_vec();
    ordered.sort_unstable_by(|left, right| right.cmp(left));
    let total: u64 = sizes.iter().sum();
    let scale = BOUNDS / total as f64;
    let areas: Vec<f64> = ordered.iter().map(|size| *size as f64 * scale).collect();

    let row_count = rows(&areas);

    (areas, row_count)
}

/// Counts rows the way the squarified algorithm would close them.
fn rows(areas: &[f64]) -> usize {
    let mut count = 1;
    let mut row = Vec::with_capacity(64);
    let mut remaining = BOUNDS;
    let mut side = 900.0f64;

    for area in areas {
        let before = worst(&row, side);
        row.push(*area);
        let after = worst(&row, side);
        if after > before && row.len() > 1 {
            let sum: f64 = row.iter().sum();
            remaining = (remaining - sum).max(1.0);
            side = remaining.sqrt().max(1.0);
            row.clear();
            row.push(*area);
            count += 1;
        }
    }

    count
}

fn worst(row: &[f64], side: f64) -> f64 {
    let sum: f64 = row.iter().sum();
    if sum <= 0.0 {
        return f64::INFINITY;
    }
    let side2 = side * side;
    let sum2 = sum * sum;

    row.iter()
        .map(|area| (side2 * area / sum2).max(sum2 / (side2 * area)))
        .fold(0.0, f64::max)
}

fn treemap_benchmarks(criterion: &mut Criterion) {
    let sizes = dataset(NODES);

    criterion.bench_function("treemap_prepare_1m", |bencher| {
        bencher.iter(|| black_box(prepare_input(black_box(&sizes))));
    });
}

criterion_group!(benches, treemap_benchmarks);
criterion_main!(benches);
