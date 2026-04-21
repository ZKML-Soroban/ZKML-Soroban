//! Model inference engine.
//!
//! Executes quantized ML models using fixed-point arithmetic. The same
//! logic runs both natively (for testing) and inside the RISC Zero zkVM
//! guest (for proof generation).

use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{DecisionTree, LogisticRegression, Model, TreeNode};

/// Run inference on a model given a vector of input features.
///
/// Returns the raw fixed-point output value.
pub fn run_inference(model: &Model, inputs: &[FixedPoint]) -> FixedPoint {
    match model {
        Model::DecisionTree(tree) => infer_decision_tree(tree, inputs),
        Model::LogisticRegression(lr) => infer_logistic_regression(lr, inputs),
        Model::TinyMLP(_mlp) => {
            // TODO: Implement MLP inference with quantized ReLU.
            unimplemented!("MLP inference is planned for a future iteration")
        }
    }
}

/// Traverse a decision tree and return the leaf value.
fn infer_decision_tree(tree: &DecisionTree, inputs: &[FixedPoint]) -> FixedPoint {
    assert_eq!(
        inputs.len(),
        tree.num_features,
        "input length must match the number of features"
    );

    let mut node_idx = 0;
    loop {
        match &tree.nodes[node_idx] {
            TreeNode::Split {
                feature_index,
                threshold,
                left,
                right,
            } => {
                if inputs[*feature_index].value <= threshold.value {
                    node_idx = *left;
                } else {
                    node_idx = *right;
                }
            }
            TreeNode::Leaf { value } => return *value,
        }
    }
}

/// Compute logistic regression output: dot(weights, inputs) + bias.
///
/// Note: The sigmoid activation is omitted because it is not
/// ZK-friendly. Instead, the verifier compares the raw linear
/// output against a threshold.
fn infer_logistic_regression(lr: &LogisticRegression, inputs: &[FixedPoint]) -> FixedPoint {
    assert_eq!(
        inputs.len(),
        lr.weights.len(),
        "input length must match the number of weights"
    );

    let scale = inputs[0].scale;
    let dot: i64 = lr
        .weights
        .iter()
        .zip(inputs.iter())
        .map(|(w, x)| (w.value * x.value) >> scale)
        .sum();

    FixedPoint::from_raw(dot + lr.bias.value, scale)
}
