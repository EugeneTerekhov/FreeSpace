//! Treemap input-preparation benchmark.
//!
//! The squarified layout itself arrives in user story 2 (`apps/freespace-gui/src/treemap`).
//! This scaffold measures the work that every treemap frame depends on:
//! sorting node sizes and computing area shares for a large directory.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

const NODES: usize = 100_000;

fn dataset(nodes: usize) -> Vec<u64> {
    (0..nodes)
        .map(|index| ((index * 7_919) % 1_000_000) as u64 + 1)
        .collect()
}

fn prepare_input(sizes: &[u64]) -> (Vec<i64>, f64) {
    let mut ordered: Vec<i64> = sizes.iter().map(|size| *size as i64).collect();
    ordered.sort_unstable_by(|left, right| right.cmp(left));
    let total: u64 = sizes.iter().sum();

    (ordered, total as f64)
}

fn treemap_benchmarks(criterion: &mut Criterion) {
    let sizes = dataset(NODES);

    criterion.bench_function("treemap_prepare_100k", |bencher| {
        bencher.iter(|| black_box(prepare_input(black_box(&sizes))));
    });
}

criterion_group!(benches, treemap_benchmarks);
criterion_main!(benches);
