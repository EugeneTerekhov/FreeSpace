//! Hashing throughput benchmark scaffold for duplicate detection.
//!
//! The duplicate pipeline (size → partial hash → full hash → byte comparison)
//! lands in user story 5. This benchmark measures the SHA-256 primitive that
//! dominates full-file hashing.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use sha2::{Digest, Sha256};

const MIB: usize = 1024 * 1024;

fn hashing_benchmarks(criterion: &mut Criterion) {
    let data = vec![0x5au8; MIB];

    criterion.bench_function("sha256_1mib", |bencher| {
        bencher.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(black_box(&data));
            black_box(hasher.finalize())
        });
    });
}

criterion_group!(benches, hashing_benchmarks);
criterion_main!(benches);
