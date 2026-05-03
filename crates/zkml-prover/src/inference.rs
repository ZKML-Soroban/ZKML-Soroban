//! Model inference engine.
//!
//! Executes quantized ML models using fixed-point arithmetic. The same
//! logic runs both natively (for testing) and inside the RISC Zero zkVM
//! guest (for proof generation).

use zkml_common::activation::relu_vec;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{
    DecisionTree, DenseLayer, LogisticRegression, Model, TinyMLP, TreeNode,
};

/// Run inference on a model given a vector of input features.
///
/// Returns the raw fixed-point output value.
pub fn run_inference(model: &Model, inputs: &[FixedPoint]) -> FixedPoint {
    match model {
        Model::DecisionTree(tree) => infer_decision_tree(tree, inputs),
        Model::LogisticRegression(lr) => infer_logistic_regression(lr, inputs),
        Model::TinyMLP(mlp) => infer_tiny_mlp(mlp, inputs),
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
            TreeNode::Split { feature_index, threshold, left, right } => {
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
/// Note: The sigmoid activation is omitted because it is not ZK-friendly.
/// Instead, the verifier compares the raw linear output against a threshold.
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

/// Compute one dense layer: `out[j] = sum_i(weight[j,i] * in[i]) + bias[j]`.
///
/// Weights are stored row-major as `weights[j * input_size + i]`.
fn dense_forward(layer: &DenseLayer, inputs: &[FixedPoint]) -> Vec<FixedPoint> {
    let scale = inputs.first().map(|x| x.scale).unwrap_or(16);
    let mut out = Vec::with_capacity(layer.output_size);
    for j in 0..layer.output_size {
        let mut acc: i64 = layer.biases[j].value;
        for i in 0..layer.input_size {
            let w = layer.weights[j * layer.input_size + i].value;
            acc += (w * inputs[i].value) >> scale;
        }
        out.push(FixedPoint::from_raw(acc, scale));
    }
    out
}

/// Run a forward pass through a tiny MLP using quantized ReLU between layers.
fn infer_tiny_mlp(mlp: &TinyMLP, inputs: &[FixedPoint]) -> FixedPoint {
    let mut activations: Vec<FixedPoint> = inputs.to_vec();
    let last = mlp.layers.len().saturating_sub(1);
    for (idx, layer) in mlp.layers.iter().enumerate() {
        let mut out = dense_forward(layer, &activations);
        if idx != last {
            out = relu_vec(&out);
        }
        activations = out;
    }
    activations.first().copied().unwrap_or(FixedPoint::from_raw(0, 16))
}
