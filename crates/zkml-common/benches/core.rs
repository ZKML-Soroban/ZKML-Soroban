//! Native benchmarks: fixed-point arithmetic, commitments and inference.
//!
//! `cargo bench -p zkml-common`. These measure the host side (the CLI, the
//! prover before it enters the zkVM); the guest's cost is its cycle count,
//! which `zkml-prover cycles` reports and CI gates.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use zkml_common::commitment::{commit_inputs, commit_model, Opening};
use zkml_common::fixed_point::{dot, FixedPoint};
use zkml_common::inference::{try_run_batch, try_run_inference};
use zkml_common::models::{DecisionTree, DenseLayer, LogisticRegression, Model, TinyMLP, TreeNode};

const OPENING: Opening = [0x2a; 32];

fn fp(x: f64) -> FixedPoint {
    FixedPoint::quantize(x)
}

/// Deterministic values in [-1, 1).
fn values(n: usize, seed: u64) -> Vec<FixedPoint> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            fp(((state >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0)
        })
        .collect()
}

fn lr(features: usize) -> Model {
    Model::LogisticRegression(LogisticRegression {
        weights: values(features, 1),
        bias: fp(0.1),
        decision_threshold: fp(0.5),
    })
}

/// A complete binary tree of `depth` over `features` inputs.
fn tree(depth: u32, features: usize) -> Model {
    let mut nodes = Vec::new();
    let internal = (1usize << depth) - 1;
    for i in 0..internal {
        nodes.push(TreeNode::Split {
            feature_index: i % features,
            threshold: fp(0.0),
            left: 2 * i + 1,
            right: 2 * i + 2,
        });
    }
    for i in 0..(1usize << depth) {
        nodes.push(TreeNode::Leaf {
            value: fp((i % 3) as f64),
        });
    }
    Model::DecisionTree(DecisionTree {
        nodes,
        num_features: features,
    })
}

fn mlp(sizes: &[usize]) -> Model {
    let layers = sizes
        .windows(2)
        .enumerate()
        .map(|(i, w)| DenseLayer {
            weights: values(w[0] * w[1], 10 + i as u64)
                .into_iter()
                .map(|v| FixedPoint::from_raw(v.value / 8, v.scale))
                .collect(),
            biases: values(w[1], 20 + i as u64),
            input_size: w[0],
            output_size: w[1],
        })
        .collect();
    Model::TinyMLP(TinyMLP { layers })
}

fn fixed_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("fixed_point");
    let (a, b) = (fp(1.25), fp(-0.75));
    group.bench_function("checked_mul", |bench| {
        bench.iter(|| black_box(a).checked_mul(black_box(b)))
    });
    group.bench_function("checked_div", |bench| {
        bench.iter(|| black_box(a).checked_div(black_box(b)))
    });
    for n in [16usize, 128, 1024] {
        let (x, y) = (values(n, 2), values(n, 3));
        group.bench_with_input(BenchmarkId::new("dot", n), &n, |bench, _| {
            bench.iter(|| dot(black_box(&x), black_box(&y)))
        });
    }
    group.finish();
}

fn commitments(c: &mut Criterion) {
    let mut group = c.benchmark_group("commitment");
    for (name, model) in [
        ("lr4", lr(4)),
        ("tree_depth5", tree(5, 10)),
        ("tree_depth12", tree(12, 10)),
        ("mlp_8_8_1", mlp(&[8, 8, 1])),
        ("mlp_64_64_10", mlp(&[64, 64, 10])),
    ] {
        group.bench_function(BenchmarkId::new("model", name), |bench| {
            bench.iter(|| commit_model(black_box(&model), &OPENING))
        });
    }
    for n in [10usize, 100, 1000] {
        let features = values(n, 4);
        group.bench_with_input(BenchmarkId::new("inputs", n), &n, |bench, _| {
            bench.iter(|| commit_inputs(black_box(&features), &OPENING))
        });
    }
    group.finish();
}

fn inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference");
    for (name, model, features) in [
        ("lr4", lr(4), 4),
        ("tree_depth5", tree(5, 10), 10),
        ("tree_depth12", tree(12, 10), 10),
        ("mlp_8_8_1", mlp(&[8, 8, 1]), 8),
    ] {
        let row = values(features, 5);
        group.bench_function(BenchmarkId::new("single", name), |bench| {
            bench.iter(|| try_run_inference(black_box(&model), black_box(&row)))
        });
        let rows: Vec<Vec<FixedPoint>> = (0..100).map(|i| values(features, 100 + i)).collect();
        group.bench_function(BenchmarkId::new("batch_100", name), |bench| {
            bench.iter(|| try_run_batch(black_box(&model), black_box(&rows)))
        });
    }
    group.finish();
}

criterion_group!(benches, fixed_point, commitments, inference);
criterion_main!(benches);
