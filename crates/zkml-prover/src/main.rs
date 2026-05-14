//! Command-line entrypoint for the zkml prover.
//!
//! Usage:
//!
//! ```text
//! zkml-prover <model.json> <comma,separated,inputs>
//! ```
//!
//! Imports a model from the JSON exchange format, runs inference on the
//! provided input vector, and prints the dequantized output.

use std::process::exit;

use zkml_common::fixed_point::FixedPoint;
use zkml_prover::inference::run_inference;
use zkml_prover::onnx::import_onnx;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: zkml-prover <model.json> <comma,separated,inputs>");
        exit(2);
    }

    let bytes = match std::fs::read(&args[1]) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("failed to read model file: {e}");
            exit(1);
        }
    };

    let model = match import_onnx(&bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("failed to import model: {e}");
            exit(1);
        }
    };

    let inputs: Vec<FixedPoint> = args[2]
        .split(',')
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .map(FixedPoint::quantize)
        .collect();

    let output = run_inference(&model, &inputs);
    println!("output: {}", output.dequantize());
}
