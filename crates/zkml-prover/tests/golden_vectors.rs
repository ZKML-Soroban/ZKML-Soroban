//! Golden-vector tests that run without the `zkvm` feature.
//!
//! These exercise decision-tree inference (including `BRANCH_LEQ` threshold
//! boundaries) in the default CI `test` job.

use serde::Deserialize;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::run_inference;
use zkml_prover::model_io::JsonModel;

#[derive(Debug, Deserialize)]
struct VectorFile {
    cases: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
struct VectorCase {
    name: String,
    model: serde_json::Value,
    inputs: Vec<f64>,
    expected_output_raw: i64,
}

fn load_tree_vectors() -> VectorFile {
    let path = format!(
        "{}/tests/vectors/decision_tree.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let data = std::fs::read_to_string(&path).expect("read vectors");
    serde_json::from_str(&data).expect("parse vectors")
}

#[test]
fn native_vectors_match_expected_raw() {
    let file = load_tree_vectors();
    for case in &file.cases {
        let model_doc: JsonModel =
            serde_json::from_value(case.model.clone()).expect("model schema");
        let model = model_doc.into_model();
        let inputs: Vec<FixedPoint> = case
            .inputs
            .iter()
            .copied()
            .map(FixedPoint::quantize)
            .collect();
        let out = run_inference(&model, &inputs);
        assert_eq!(
            out.value, case.expected_output_raw,
            "case {} native mismatch",
            case.name
        );
    }
}
