//! Model inference engine.
//!
//! Executes quantized ML models using fixed-point arithmetic. The same
//! logic runs both natively (for testing) and inside the RISC Zero zkVM
//! guest (for proof generation). Keeping inference in `zkml-common` avoids
//! duplicating the proven path between host and guest.

use crate::activation::relu_vec;
use crate::error::ZkmlError;
use crate::fixed_point::FixedPoint;
use crate::models::{DecisionTree, DenseLayer, LogisticRegression, Model, TinyMLP, TreeNode};

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
///
/// # Threshold semantics
///
/// A sample takes the **left** child when
/// `feature[feature_index].value <= threshold.value` (inclusive / `BRANCH_LEQ`).
/// Values strictly greater than the threshold take the right child. This
/// matches typical ONNX `TreeEnsembleClassifier` `BRANCH_LEQ` behavior and
/// must stay aligned with any future circuit encoding.
///
/// # Panics
///
/// Panics if input length doesn't match expected features, or if the tree
/// has a cycle causing the iteration limit to be exceeded. Prefer
/// [`try_infer_decision_tree`] when cycle detection must be handled as an error.
fn infer_decision_tree(tree: &DecisionTree, inputs: &[FixedPoint]) -> FixedPoint {
    try_infer_decision_tree(tree, inputs).expect("decision tree inference exceeded iteration limit")
}

/// Fallible decision tree inference with bounded iteration.
///
/// Traverses the tree with a maximum iteration count to prevent infinite loops
/// from malformed trees. Returns an error if the iteration limit is exceeded.
fn try_infer_decision_tree(
    tree: &DecisionTree,
    inputs: &[FixedPoint],
) -> Result<FixedPoint, ZkmlError> {
    if inputs.len() != tree.num_features {
        return Err(ZkmlError::FeatureCountMismatch {
            expected: tree.num_features,
            got: inputs.len(),
        });
    }

    const MAX_ITERATIONS: usize = 10_000;
    let mut node_idx = 0;
    for _iteration in 0..MAX_ITERATIONS {
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
            TreeNode::Leaf { value } => return Ok(*value),
        }
    }

    Err(ZkmlError::InvalidModel(format!(
        "decision tree traversal exceeded maximum iterations ({MAX_ITERATIONS}) - possible cycle"
    )))
}

/// Fallible LogisticRegression forward used by [`try_run_inference`].
///
/// Computes `dot(weights, inputs) + bias` using checked `i128` arithmetic to
/// prevent overflow, matching [`dense_forward`]. Validates that inputs are
/// non-empty and share a uniform fixed-point scale.
fn try_infer_logistic_regression(
    lr: &LogisticRegression,
    inputs: &[FixedPoint],
) -> Result<FixedPoint, ZkmlError> {
    if inputs.is_empty() {
        return Err(ZkmlError::FeatureCountMismatch {
            expected: lr.weights.len(),
            got: 0,
        });
    }

    if inputs.len() != lr.weights.len() {
        return Err(ZkmlError::FeatureCountMismatch {
            expected: lr.weights.len(),
            got: inputs.len(),
        });
    }

    let scale = inputs[0].scale;
    if inputs.iter().any(|x| x.scale != scale) {
        return Err(ZkmlError::QuantizationError(
            "inputs have non-uniform fixed-point scale".to_string(),
        ));
    }

    let mut acc = lr.bias;
    for (w, x) in lr.weights.iter().zip(inputs.iter()) {
        let product = w.checked_mul(*x).ok_or(ZkmlError::ArithmeticOverflow)?;
        acc = acc
            .checked_add(product)
            .ok_or(ZkmlError::ArithmeticOverflow)?;
    }

    Ok(FixedPoint::from_raw(acc.value, scale))
}

/// Compute logistic regression output: dot(weights, inputs) + bias.
///
/// Note: The sigmoid activation is omitted because it is not ZK-friendly.
/// Instead, the verifier compares the raw linear output against a threshold.
///
/// # Panics
///
/// Panics if a checked fixed-point multiply or add overflows, or if input validation fails.
/// Prefer [`try_run_inference`] when overflow must be handled as an error.
fn infer_logistic_regression(lr: &LogisticRegression, inputs: &[FixedPoint]) -> FixedPoint {
    try_infer_logistic_regression(lr, inputs).expect("Logistic regression inference overflow")
}

/// Compute one dense layer: `out[j] = sum_i(weight[j,i] * in[i]) + bias[j]`.
///
/// Weights are stored row-major as `weights[j * input_size + i]`.
/// Products use [`FixedPoint::checked_mul`] (i128 intermediate) and the
/// accumulator uses [`FixedPoint::checked_add`].
#[allow(clippy::needless_range_loop)]
fn dense_forward(layer: &DenseLayer, inputs: &[FixedPoint]) -> Result<Vec<FixedPoint>, ZkmlError> {
    let scale = inputs.first().map(|x| x.scale).unwrap_or(16);
    let mut out = Vec::with_capacity(layer.output_size);
    for j in 0..layer.output_size {
        let mut acc = layer.biases[j];
        for (i, x) in inputs.iter().enumerate().take(layer.input_size) {
            let w = layer.weights[j * layer.input_size + i];
            let product = w.checked_mul(*x).ok_or(ZkmlError::ArithmeticOverflow)?;
            acc = acc
                .checked_add(product)
                .ok_or(ZkmlError::ArithmeticOverflow)?;
        }
        // Preserve caller scale when bias/weights share it (the normal case).
        out.push(FixedPoint::from_raw(acc.value, scale));
    }
    Ok(out)
}

/// Run a forward pass through a tiny MLP using quantized ReLU between layers.
///
/// # Activation convention
///
/// Quantized ReLU (`max(0, x)`) is applied after **every layer except the
/// last**. The final layer returns raw linear scores (no sigmoid/softmax),
/// matching logistic regression's "omit the sigmoid" convention.
/// [`run_inference`] exposes the first output neuron of that final layer;
/// multi-class callers can use [`argmax`] on a full logits vector when the
/// layer width is greater than one.
///
/// # Panics
///
/// Panics if a checked fixed-point multiply or add overflows. Prefer
/// [`try_run_inference`] when overflow must be handled as an error.
fn infer_tiny_mlp(mlp: &TinyMLP, inputs: &[FixedPoint]) -> FixedPoint {
    try_infer_tiny_mlp(mlp, inputs).expect("TinyMLP inference overflow")
}

/// Fallible TinyMLP forward used by [`try_run_inference`].
///
/// See [`infer_tiny_mlp`] for the ReLU-after-hidden / raw-final-layer
/// convention.
fn try_infer_tiny_mlp(mlp: &TinyMLP, inputs: &[FixedPoint]) -> Result<FixedPoint, ZkmlError> {
    let mut activations: Vec<FixedPoint> = inputs.to_vec();
    let last = mlp.layers.len().saturating_sub(1);
    for (idx, layer) in mlp.layers.iter().enumerate() {
        let mut out = dense_forward(layer, &activations)?;
        if idx != last {
            // Quantized ReLU after every hidden layer. The single shared
            // implementation lives in `crate::activation` so the native
            // prover and the guest apply the exact same activation.
            out = relu_vec(&out);
        }
        activations = out;
    }
    Ok(activations
        .first()
        .copied()
        .unwrap_or(FixedPoint::from_raw(0, 16)))
}

#[cfg(test)]
mod tests_mlp {
    use super::*;
    use crate::models::{DenseLayer, Model, TinyMLP};

    fn fp(x: f64) -> FixedPoint {
        FixedPoint::quantize(x)
    }

    #[test]
    fn single_layer_identity() {
        // One input, one output, weight 1.0, bias 0.0 -> output equals input.
        let layer = DenseLayer {
            weights: vec![fp(1.0)],
            biases: vec![fp(0.0)],
            input_size: 1,
            output_size: 1,
        };
        let model = Model::TinyMLP(TinyMLP {
            layers: vec![layer],
        });
        let out = run_inference(&model, &[fp(0.7)]);
        assert!((out.dequantize() - 0.7).abs() < 1e-2);
    }
}

/// Return the index of the largest value in a fixed-point vector.
///
/// Used to turn a multi-output MLP layer into a class label without a
/// (ZK-unfriendly) softmax: argmax of the logits equals argmax of softmax.
pub fn argmax(values: &[FixedPoint]) -> Option<usize> {
    values
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| v.value)
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests_argmax {
    use super::*;

    #[test]
    fn argmax_picks_highest_logit() {
        let logits = vec![
            FixedPoint::quantize(0.1),
            FixedPoint::quantize(0.9),
            FixedPoint::quantize(0.4),
        ];
        assert_eq!(argmax(&logits), Some(1));
    }
}

/// Run inference for each input row, returning one output per row.
pub fn run_batch(model: &Model, rows: &[Vec<FixedPoint>]) -> Vec<FixedPoint> {
    rows.iter().map(|row| run_inference(model, row)).collect()
}

#[cfg(test)]
mod tests_batch {
    use super::*;
    use crate::models::{LogisticRegression, Model};

    #[test]
    fn batch_matches_single() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::quantize(1.0)],
            bias: FixedPoint::quantize(0.0),
        });
        let rows = vec![
            vec![FixedPoint::quantize(0.5)],
            vec![FixedPoint::quantize(0.9)],
        ];
        let batched = run_batch(&model, &rows);
        for (row, out) in rows.iter().zip(batched.iter()) {
            assert_eq!(run_inference(&model, row).value, out.value);
        }
    }
}

/// Validated inference that returns an error instead of panicking on a
/// feature-count mismatch, empty input, invalid TinyMLP topology, or
/// fixed-point overflow.
pub fn try_run_inference(model: &Model, inputs: &[FixedPoint]) -> Result<FixedPoint, ZkmlError> {
    if inputs.is_empty() {
        return Err(ZkmlError::FeatureCountMismatch {
            expected: model.num_features(),
            got: 0,
        });
    }
    let expected = model.num_features();
    if expected != 0 && inputs.len() != expected {
        return Err(ZkmlError::FeatureCountMismatch {
            expected,
            got: inputs.len(),
        });
    }
    match model {
        Model::LogisticRegression(lr) => try_infer_logistic_regression(lr, inputs),
        Model::TinyMLP(mlp) => {
            mlp.validate()?;
            try_infer_tiny_mlp(mlp, inputs)
        }
        Model::DecisionTree(tree) => try_infer_decision_tree(tree, inputs),
    }
}

#[cfg(test)]
mod tests_validated {
    use super::*;
    use crate::models::{LogisticRegression, Model};

    #[test]
    fn empty_input_is_rejected() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::quantize(1.0)],
            bias: FixedPoint::quantize(0.0),
        });
        assert!(try_run_inference(&model, &[]).is_err());
    }

    #[test]
    fn logistic_regression_overflow_boundary_returns_error() {
        let big = FixedPoint::from_raw(i64::MAX / 2, 16);
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![big],
            bias: FixedPoint::quantize(0.0),
        });
        let inputs = vec![big];
        assert_eq!(
            try_run_inference(&model, &inputs),
            Err(ZkmlError::ArithmeticOverflow)
        );
    }

    #[test]
    fn logistic_regression_mixed_scale_returns_error() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::from_raw(100, 16), FixedPoint::from_raw(100, 16)],
            bias: FixedPoint::from_raw(0, 16),
        });
        let inputs = vec![FixedPoint::from_raw(100, 16), FixedPoint::from_raw(100, 8)];
        assert!(matches!(
            try_run_inference(&model, &inputs),
            Err(ZkmlError::QuantizationError(_))
        ));
    }

    #[test]
    fn logistic_regression_in_range_parity() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::quantize(2.5), FixedPoint::quantize(-1.5)],
            bias: FixedPoint::quantize(0.5),
        });
        let inputs = vec![FixedPoint::quantize(4.0), FixedPoint::quantize(2.0)];
        let res = try_run_inference(&model, &inputs).unwrap();
        assert!((res.dequantize() - 7.5).abs() < 1e-3);
        assert_eq!(run_inference(&model, &inputs).value, res.value);
    }
}
