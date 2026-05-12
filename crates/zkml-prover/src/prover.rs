//! Proof generation module.
//!
//! Orchestrates the end-to-end proof pipeline:
//!
//! 1. Accept a quantized model and input features.
//! 2. Commit to the model and the inputs.
//! 3. Execute inference inside the RISC Zero zkVM guest (Phase 1).
//! 4. Wrap the result into a Groth16 `VerificationBundle`.

use zkml_common::commitment::{commit_i64, Commitment};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{Model, TreeNode};
use zkml_common::proof::VerificationBundle;

/// Flatten a model's fixed-point parameters into field elements so they can be
/// committed to deterministically.
fn model_elements(model: &Model) -> Vec<i64> {
    let mut out = Vec::new();
    match model {
        Model::LogisticRegression(lr) => {
            out.extend(lr.weights.iter().map(|w| w.value));
            out.push(lr.bias.value);
        }
        Model::DecisionTree(tree) => {
            out.push(tree.num_features as i64);
            for node in &tree.nodes {
                match node {
                    TreeNode::Split { feature_index, threshold, left, right } => {
                        out.push(*feature_index as i64);
                        out.push(threshold.value);
                        out.push(*left as i64);
                        out.push(*right as i64);
                    }
                    TreeNode::Leaf { value } => out.push(value.value),
                }
            }
        }
        Model::TinyMLP(mlp) => {
            for layer in &mlp.layers {
                out.extend(layer.weights.iter().map(|w| w.value));
                out.extend(layer.biases.iter().map(|b| b.value));
                out.push(layer.input_size as i64);
                out.push(layer.output_size as i64);
            }
        }
    }
    out
}

/// Commit to a model's parameters (the on-chain `initialize` value).
pub fn model_commitment(model: &Model) -> Commitment {
    commit_i64(&model_elements(model))
}

/// Commit to a set of input features (a proof public input).
pub fn input_commitment(inputs: &[FixedPoint]) -> Commitment {
    let elements: Vec<i64> = inputs.iter().map(|x| x.value).collect();
    commit_i64(&elements)
}

/// Generate a ZK proof attesting that `model` produces some output on `inputs`.
///
/// # Errors
///
/// Returns an error string if proof generation fails.
pub fn generate_proof(
    _model: &Model,
    _inputs: &[FixedPoint],
) -> Result<VerificationBundle, String> {
    // TODO: Integrate the RISC Zero zkVM proving pipeline (see docs/proving.md).
    Err("Proof generation is not yet implemented".to_string())
}
