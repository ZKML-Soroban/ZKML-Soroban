//! JSON model exchange format.
//!
//! Until full ONNX protobuf support lands, models are imported from a simple
//! JSON representation that mirrors the `zkml-common` model types. Tools that
//! export from scikit-learn or PyTorch can target this schema directly.

use serde::{Deserialize, Serialize};

/// Top-level JSON model document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JsonModel {
    LogisticRegression { weights: Vec<f64>, bias: f64 },
    DecisionTree { num_features: usize, nodes: Vec<JsonTreeNode> },
    TinyMlp { layers: Vec<JsonDenseLayer> },
}

/// JSON representation of a decision-tree node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JsonTreeNode {
    Split { feature_index: usize, threshold: f64, left: usize, right: usize },
    Leaf { value: f64 },
}

/// JSON representation of a dense layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonDenseLayer {
    pub weights: Vec<f64>,
    pub biases: Vec<f64>,
    pub input_size: usize,
    pub output_size: usize,
}

use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{LogisticRegression, Model};

impl JsonModel {
    /// Convert a logistic regression JSON document into the internal model.
    fn into_logistic(weights: Vec<f64>, bias: f64) -> Model {
        Model::LogisticRegression(LogisticRegression {
            weights: weights.iter().copied().map(FixedPoint::quantize).collect(),
            bias: FixedPoint::quantize(bias),
        })
    }
}

use zkml_common::models::{DecisionTree, TreeNode};

impl JsonModel {
    /// Convert decision-tree JSON nodes into the internal flat node vector.
    fn into_tree(num_features: usize, nodes: Vec<JsonTreeNode>) -> Model {
        let nodes = nodes
            .into_iter()
            .map(|n| match n {
                JsonTreeNode::Split { feature_index, threshold, left, right } => {
                    TreeNode::Split {
                        feature_index,
                        threshold: FixedPoint::quantize(threshold),
                        left,
                        right,
                    }
                }
                JsonTreeNode::Leaf { value } => {
                    TreeNode::Leaf { value: FixedPoint::quantize(value) }
                }
            })
            .collect();
        Model::DecisionTree(DecisionTree { nodes, num_features })
    }
}

use zkml_common::models::{DenseLayer, TinyMLP};

impl JsonModel {
    /// Convert a tiny MLP JSON document into the internal model.
    fn into_mlp(layers: Vec<JsonDenseLayer>) -> Model {
        let layers = layers
            .into_iter()
            .map(|l| DenseLayer {
                weights: l.weights.iter().copied().map(FixedPoint::quantize).collect(),
                biases: l.biases.iter().copied().map(FixedPoint::quantize).collect(),
                input_size: l.input_size,
                output_size: l.output_size,
            })
            .collect();
        Model::TinyMLP(TinyMLP { layers })
    }

    /// Lower any JSON model into the internal `Model` representation.
    pub fn into_model(self) -> Model {
        match self {
            JsonModel::LogisticRegression { weights, bias } => {
                Self::into_logistic(weights, bias)
            }
            JsonModel::DecisionTree { num_features, nodes } => {
                Self::into_tree(num_features, nodes)
            }
            JsonModel::TinyMlp { layers } => Self::into_mlp(layers),
        }
    }
}
