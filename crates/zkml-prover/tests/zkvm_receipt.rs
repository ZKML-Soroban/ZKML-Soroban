//! Integration tests for RISC Zero guest inference receipts.
//!
//! Default tests expect `RISC0_DEV_MODE=1` (fake proofs, fast). Real proving
//! is gated behind `#[ignore]` — see `crates/zkml-prover/README.md`.
//!
//! Native golden vectors live in `golden_vectors.rs` so they run without the
//! `zkvm` feature in the default CI job.

#![cfg(feature = "zkvm")]

use serde::Deserialize;
use zkml_common::fixed_point::FixedPoint;
use zkml_prover::model_io::JsonModel;
use zkml_prover::prover::{generate_receipt, input_commitment, model_commitment};

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

fn case_to_model_inputs(case: &VectorCase) -> (zkml_common::models::Model, Vec<FixedPoint>) {
    let model_doc: JsonModel = serde_json::from_value(case.model.clone()).expect("model schema");
    let model = model_doc.into_model();
    let inputs: Vec<FixedPoint> = case
        .inputs
        .iter()
        .copied()
        .map(FixedPoint::quantize)
        .collect();
    (model, inputs)
}

#[test]
fn dev_mode_receipt_matches_native_for_all_tree_vectors() {
    // Dev-mode is required for CI-friendly runtimes.
    // SAFETY: single-threaded test process; setting env for prover backend.
    std::env::set_var("RISC0_DEV_MODE", "1");

    let file = load_tree_vectors();
    for case in &file.cases {
        let (model, inputs) = case_to_model_inputs(case);
        let (_receipt, journal) =
            generate_receipt(&model, &inputs).unwrap_or_else(|e| panic!("case {}: {e}", case.name));

        assert_eq!(
            journal.output, case.expected_output_raw,
            "case {} journal output",
            case.name
        );
        assert_eq!(journal.model_hash, model_commitment(&model));
        assert_eq!(journal.input_hash, input_commitment(&inputs));
    }
}

/// Real proving (non-dev-mode). Slow; run manually:
///
/// ```text
/// RISC0_DEV_MODE=0 cargo test -p zkml-prover --test zkvm_receipt real_proof -- --ignored --nocapture
/// ```
#[test]
#[ignore = "real RISC Zero proving is slow; run manually with RISC0_DEV_MODE unset or 0"]
fn real_proof_decision_tree_stump() {
    std::env::remove_var("RISC0_DEV_MODE");
    // Explicitly disable dev mode if the env was inherited.
    std::env::set_var("RISC0_DEV_MODE", "0");

    let file = load_tree_vectors();
    let case = file
        .cases
        .iter()
        .find(|c| c.name == "depth1_left_branch")
        .expect("depth1_left_branch vector");
    let (model, inputs) = case_to_model_inputs(case);
    let (_receipt, journal) = generate_receipt(&model, &inputs).expect("real prove");
    assert_eq!(journal.output, case.expected_output_raw);
}
