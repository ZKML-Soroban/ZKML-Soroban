//! Integration test: import a bundled example model and run inference.

use zkml_common::fixed_point::FixedPoint;
use zkml_prover::inference::run_inference;
use zkml_prover::model_io::import_json;

#[test]
fn runs_credit_example() {
    let bytes = include_str!("../../../examples/models/credit_lr.json").as_bytes();
    let model = import_json(bytes).expect("model imports cleanly");

    let inputs: Vec<FixedPoint> = [0.5, 0.2, 0.9, 0.1]
        .iter()
        .map(|&x| FixedPoint::quantize(x))
        .collect();

    let output = run_inference(&model, &inputs);
    // Just assert it runs and produces a finite value.
    assert!(output.dequantize().is_finite());
}
