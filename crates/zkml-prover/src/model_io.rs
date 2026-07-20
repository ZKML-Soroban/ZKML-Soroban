//! JSON model exchange format.
//!
//! Companion to the ONNX protobuf importer in [`crate::onnx`]. JSON remains the
//! practical path for demos and golden models under `examples/models/` while
//! ONNX parameter extraction (issues #5 / #6) is incomplete.
//!
//! Tools that export from scikit-learn or PyTorch can target this schema
//! directly; see `docs/model-format.md`.

use serde::{Deserialize, Serialize};

/// Top-level JSON model document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JsonModel {
    LogisticRegression {
        weights: Vec<f64>,
        bias: f64,
    },
    DecisionTree {
        num_features: usize,
        nodes: Vec<JsonTreeNode>,
    },
    TinyMlp {
        layers: Vec<JsonDenseLayer>,
    },
}

/// JSON representation of a decision-tree node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JsonTreeNode {
    Split {
        feature_index: usize,
        threshold: f64,
        left: usize,
        right: usize,
    },
    Leaf {
        value: f64,
    },
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

/// Import a model from the JSON exchange format.
///
/// # Errors
///
/// Returns a descriptive error string if the bytes cannot be parsed as JSON
/// matching [`JsonModel`].
pub fn import_json(bytes: &[u8]) -> Result<Model, String> {
    let doc: JsonModel =
        serde_json::from_slice(bytes).map_err(|e| format!("model parse error: {e}"))?;
    Ok(doc.into_model())
}

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
                JsonTreeNode::Split {
                    feature_index,
                    threshold,
                    left,
                    right,
                } => TreeNode::Split {
                    feature_index,
                    threshold: FixedPoint::quantize(threshold),
                    left,
                    right,
                },
                JsonTreeNode::Leaf { value } => TreeNode::Leaf {
                    value: FixedPoint::quantize(value),
                },
            })
            .collect();
        Model::DecisionTree(DecisionTree {
            nodes,
            num_features,
        })
    }
}

use zkml_common::models::{DenseLayer, TinyMLP};

impl JsonModel {
    /// Convert a tiny MLP JSON document into the internal model.
    fn into_mlp(layers: Vec<JsonDenseLayer>) -> Model {
        let layers = layers
            .into_iter()
            .map(|l| DenseLayer {
                weights: l
                    .weights
                    .iter()
                    .copied()
                    .map(FixedPoint::quantize)
                    .collect(),
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
            JsonModel::LogisticRegression { weights, bias } => Self::into_logistic(weights, bias),
            JsonModel::DecisionTree {
                num_features,
                nodes,
            } => Self::into_tree(num_features, nodes),
            JsonModel::TinyMlp { layers } => Self::into_mlp(layers),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_logistic_regression() {
        let json = r#"{"kind":"logistic_regression","weights":[0.5,-0.5],"bias":0.0}"#;
        let doc: JsonModel = serde_json::from_str(json).unwrap();
        match doc.into_model() {
            Model::LogisticRegression(lr) => assert_eq!(lr.weights.len(), 2),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_tree_round_trip() {
        let json = r#"{"kind":"decision_tree","num_features":1,
            "nodes":[{"type":"leaf","value":1.0}]}"#;
        let doc: JsonModel = serde_json::from_str(json).unwrap();
        match doc.into_model() {
            Model::DecisionTree(t) => assert_eq!(t.nodes.len(), 1),
            _ => panic!("wrong variant"),
        }
    }
}
