//! Model representation types.
//!
//! Defines the intermediate representations for supported ML model
//! architectures. These structures are produced by the ONNX importer
//! in the prover crate and consumed by both the inference engine and
//! the circuit generator.

use serde::{Deserialize, Serialize};

use crate::fixed_point::FixedPoint;

// ---------------------------------------------------------------------------
// Decision Tree
// ---------------------------------------------------------------------------

/// A single node in a binary decision tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TreeNode {
    /// Internal split node.
    Split {
        /// Index of the feature to evaluate.
        feature_index: usize,
        /// Threshold value (fixed-point).
        threshold: FixedPoint,
        /// Index of the left child node.
        left: usize,
        /// Index of the right child node.
        right: usize,
    },
    /// Terminal leaf node.
    Leaf {
        /// The predicted class label or regression value.
        value: FixedPoint,
    },
}

/// A complete decision tree represented as a flat node vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTree {
    /// Flat array of tree nodes; index 0 is the root.
    pub nodes: Vec<TreeNode>,
    /// Number of input features expected.
    pub num_features: usize,
}

// ---------------------------------------------------------------------------
// Logistic Regression
// ---------------------------------------------------------------------------

/// A binary logistic regression model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogisticRegression {
    /// Weight vector (one per feature), in fixed-point.
    pub weights: Vec<FixedPoint>,
    /// Bias term, in fixed-point.
    pub bias: FixedPoint,
}

// ---------------------------------------------------------------------------
// Tiny MLP
// ---------------------------------------------------------------------------

/// A single dense (fully connected) layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenseLayer {
    /// Weight matrix stored in row-major order: `weights[i * in + j]`.
    pub weights: Vec<FixedPoint>,
    /// Bias vector, one entry per output neuron.
    pub biases: Vec<FixedPoint>,
    /// Number of input neurons.
    pub input_size: usize,
    /// Number of output neurons.
    pub output_size: usize,
}

/// A small multi-layer perceptron with quantized ReLU activations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TinyMLP {
    /// Ordered list of dense layers.
    pub layers: Vec<DenseLayer>,
}

// ---------------------------------------------------------------------------
// Unified enum
// ---------------------------------------------------------------------------

/// Enum over supported model architectures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Model {
    DecisionTree(DecisionTree),
    LogisticRegression(LogisticRegression),
    TinyMLP(TinyMLP),
}
