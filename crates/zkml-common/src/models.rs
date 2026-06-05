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

impl DecisionTree {
    /// Validate that every split node references in-bounds children and
    /// feature indices. Returns an error describing the first problem found.
    pub fn validate(&self) -> Result<(), crate::error::ZkmlError> {
        for (i, node) in self.nodes.iter().enumerate() {
            if let TreeNode::Split { feature_index, left, right, .. } = node {
                if *feature_index >= self.num_features {
                    return Err(crate::error::ZkmlError::InvalidModel(format!(
                        "node {i}: feature index {feature_index} out of range"
                    )));
                }
                if *left >= self.nodes.len() || *right >= self.nodes.len() {
                    return Err(crate::error::ZkmlError::InvalidModel(format!(
                        "node {i}: child index out of range"
                    )));
                }
            }
        }
        Ok(())
    }
}

impl LogisticRegression {
    /// Number of input features this model expects.
    pub fn num_features(&self) -> usize {
        self.weights.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed_point::FixedPoint;

    #[test]
    fn valid_tree_passes() {
        let tree = DecisionTree {
            num_features: 1,
            nodes: vec![
                TreeNode::Split {
                    feature_index: 0,
                    threshold: FixedPoint::quantize(0.5),
                    left: 1,
                    right: 2,
                },
                TreeNode::Leaf { value: FixedPoint::quantize(0.0) },
                TreeNode::Leaf { value: FixedPoint::quantize(1.0) },
            ],
        };
        assert!(tree.validate().is_ok());
    }

    #[test]
    fn out_of_range_child_fails() {
        let tree = DecisionTree {
            num_features: 1,
            nodes: vec![TreeNode::Split {
                feature_index: 0,
                threshold: FixedPoint::quantize(0.5),
                left: 9,
                right: 2,
            }],
        };
        assert!(tree.validate().is_err());
    }
}

impl Model {
    /// Number of input features this model expects.
    pub fn num_features(&self) -> usize {
        match self {
            Model::DecisionTree(t) => t.num_features,
            Model::LogisticRegression(lr) => lr.weights.len(),
            Model::TinyMLP(m) => m.layers.first().map(|l| l.input_size).unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests_features {
    use super::*;
    use crate::fixed_point::FixedPoint;

    #[test]
    fn logistic_feature_count() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::quantize(0.1); 5],
            bias: FixedPoint::quantize(0.0),
        });
        assert_eq!(model.num_features(), 5);
    }
}
