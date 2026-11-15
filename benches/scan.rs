//! Scan-throughput benchmark scaffold.
//!
//! The MFT and directory-walk scanners arrive in user story 1. Until then the
//! benchmark measures the part of the pipeline that already exists: building
//! the arena-backed tree with per-node size metrics, which is the dominant
//! memory-bound cost of a scan.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use freespace_core::{
    size::SizeMetrics,
    tree::{FsTree, NodeKind, NodeSpec},
};

const NODES: usize = 100_000;

fn build_tree(nodes: usize) -> FsTree {
    let mut tree = FsTree::with_capacity(nodes + 1, nodes * 24);
    let root = tree
        .add_node(
            None,
            NodeSpec {
                name: r"C:\".to_owned(),
                kind: NodeKind::Directory,
                ..NodeSpec::default()
            },
        )
        .expect("root node");

    for index in 0..nodes {
        let parent = if index % 10 == 0 { root } else { index as u32 };
        tree.add_node(
            Some(parent),
            NodeSpec {
                name: format!("file-{index:07}.dat"),
                kind: NodeKind::File,
                size: SizeMetrics::exact(4_096, 4_096, 4_096, 0),
                file_ref: index as u64,
                ..NodeSpec::default()
            },
        )
        .expect("child node");
    }

    tree
}

fn tree_benchmarks(criterion: &mut Criterion) {
    criterion.bench_function("tree_build_100k", |bencher| {
        bencher.iter(|| black_box(build_tree(NODES)));
    });

    let tree = build_tree(NODES);
    criterion.bench_function("tree_walk_100k", |bencher| {
        bencher.iter(|| {
            let mut total = SizeMetrics::ZERO;
            let mut stack: Vec<_> = tree.roots().collect();
            while let Some(node) = stack.pop() {
                total += tree.node(node).size;
                stack.extend(tree.children(node));
            }
            black_box(total)
        });
    });
}

criterion_group!(benches, tree_benchmarks);
criterion_main!(benches);
