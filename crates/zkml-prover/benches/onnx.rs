//! ONNX import, from protobuf bytes to a quantized model.
//!
//! `cargo bench -p zkml-prover`. Uses the fixtures the import tests use,
//! including a file written by skl2onnx itself.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use zkml_prover::onnx::import_onnx;

const FIXTURES: &[&str] = &[
    "linear_classifier_valid.onnx",
    "skl2onnx_real_tree.onnx",
    "onnx_helper_mlp.onnx",
];

fn onnx_import(c: &mut Criterion) {
    let mut group = c.benchmark_group("onnx_import");
    for name in FIXTURES {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        import_onnx(&bytes).unwrap_or_else(|e| panic!("{name} must import: {e}"));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |bench, bytes| {
            bench.iter(|| import_onnx(black_box(bytes)))
        });
    }
    group.finish();
}

criterion_group!(benches, onnx_import);
criterion_main!(benches);
