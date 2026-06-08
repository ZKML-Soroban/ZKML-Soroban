//! Proof generation module.
//!
//! Orchestrates the end-to-end proof pipeline: commit to the model and inputs,
//! run inference, and package the result into a `VerificationBundle` ready for
//! on-chain verification.

use zkml_common::commitment::{commit_i64, Commitment};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{Model, TreeNode};
use zkml_common::proof::{Groth16Proof, PublicInputs, VerificationBundle};

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

/// Generate a verification bundle for `model` evaluated on `inputs`.
///
/// The Groth16 proof bytes are a Phase 1 placeholder; the public inputs and
/// commitments are fully computed so the on-chain interface can be exercised
/// end to end.
pub fn generate_proof(
    model: &Model,
    inputs: &[FixedPoint],
) -> Result<VerificationBundle, String> {
    let output = crate::inference::run_inference(model, inputs);

    let public_inputs = PublicInputs {
        model_hash: model_commitment(model),
        input_hash: input_commitment(inputs),
        output: output.value.to_le_bytes().to_vec(),
    };

    // TODO: replace with a real RISC Zero receipt lowered to Groth16.
    let proof = Groth16Proof { data: Vec::new() };

    Ok(VerificationBundle { proof, public_inputs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zkml_common::models::LogisticRegression;

    fn fp(x: f64) -> FixedPoint {
        FixedPoint::quantize(x)
    }

    #[test]
    fn bundle_is_populated() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![fp(0.5), fp(-0.25)],
            bias: fp(0.1),
        });
        let inputs = vec![fp(1.0), fp(2.0)];
        let bundle = generate_proof(&model, &inputs).unwrap();
        assert_ne!(bundle.public_inputs.model_hash, [0u8; 32]);
        assert_eq!(bundle.public_inputs.output.len(), 8);
    }
}

/// Serialize a verification bundle to a JSON string.
pub fn bundle_to_json(bundle: &VerificationBundle) -> Result<String, String> {
    serde_json::to_string(bundle).map_err(|e| e.to_string())
}

/// Deserialize a verification bundle from a JSON string.
pub fn bundle_from_json(s: &str) -> Result<VerificationBundle, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests_json {
    use super::*;
    use zkml_common::models::LogisticRegression;

    #[test]
    fn bundle_json_round_trips() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::quantize(0.5)],
            bias: FixedPoint::quantize(0.0),
        });
        let bundle = generate_proof(&model, &[FixedPoint::quantize(1.0)]).unwrap();
        let json = bundle_to_json(&bundle).unwrap();
        let restored = bundle_from_json(&json).unwrap();
        assert_eq!(restored.public_inputs.model_hash, bundle.public_inputs.model_hash);
    }
}
