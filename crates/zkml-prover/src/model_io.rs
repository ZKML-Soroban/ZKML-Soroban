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
