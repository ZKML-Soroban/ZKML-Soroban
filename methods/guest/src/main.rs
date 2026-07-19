//! zkml inference guest program.
//!
//! Reads a quantized model and input features from the host, runs the shared
//! `zkml_common::inference` engine, and commits public journal fields:
//! `(model_hash, input_hash, output_raw)`.
//!
//! STARK → Groth16 compression is out of scope here (issue #11).

#![no_main]

use risc0_zkvm::guest::env;
use serde::{Deserialize, Serialize};
use zkml_common::commitment::{commitment_hash, Commitment};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::run_inference;
use zkml_common::models::{Model, TreeNode};

risc0_zkvm::guest::entry!(main);

/// Public journal payload committed by the guest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuestJournal {
    pub model_hash: Commitment,
    pub input_hash: Commitment,
    pub output: i64,
}

fn main() {
    let model: Model = env::read();
    let inputs: Vec<FixedPoint> = env::read();

    let output = run_inference(&model, &inputs);

    let model_hash = commitment_hash(&model_elements(&model));
    let input_hash = commitment_hash(&inputs.iter().map(|x| x.value).collect::<Vec<_>>());

    env::commit(&GuestJournal {
        model_hash,
        input_hash,
        output: output.value,
    });
}

/// Flatten model parameters into the same element stream the host uses for
/// `model_commitment`. Must stay in lockstep with `zkml_prover::prover`.
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
                    TreeNode::Split {
                        feature_index,
                        threshold,
                        left,
                        right,
                    } => {
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
