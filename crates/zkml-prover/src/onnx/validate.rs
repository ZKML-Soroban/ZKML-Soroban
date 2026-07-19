//! Opset and operator validation for ONNX models.

use super::error::OnnxImportError;
use super::proto::{GraphProto, ModelProto};

/// Domains whose opset version we enforce against the minimum.
///
/// - empty / `ai.onnx`: core operators (`MatMul`, `Add`, `Relu`, …)
/// - `ai.onnx.ml`: classic ML operators (`TreeEnsembleClassifier`,
///   `LinearClassifier`)
fn is_relevant_domain(domain: &str) -> bool {
    domain.is_empty() || domain == "ai.onnx" || domain == "ai.onnx.ml"
}

/// Ensure every relevant opset import is at least `min_version`.
///
/// If the model declares no opset imports at all, treat that as malformed:
/// well-formed ONNX models always list their opset requirements.
pub fn check_opset(model: &ModelProto, min_version: i64) -> Result<(), OnnxImportError> {
    if model.opset_import.is_empty() {
        return Err(OnnxImportError::MalformedModel(
            "model declares no opset_import entries".into(),
        ));
    }

    let mut saw_relevant = false;
    for entry in &model.opset_import {
        if !is_relevant_domain(&entry.domain) {
            continue;
        }
        saw_relevant = true;
        if entry.version < min_version {
            return Err(OnnxImportError::UnsupportedOpset {
                found: entry.version,
                required: min_version,
            });
        }
    }

    // If only exotic domains were listed, still require an explicit core/ml
    // opset so we never silently accept an unspecified dialect.
    if !saw_relevant {
        return Err(OnnxImportError::MalformedModel(
            "model has no opset_import for ai.onnx or ai.onnx.ml".into(),
        ));
    }

    Ok(())
}

/// Walk every graph node and reject the first operator outside `allowed`.
pub fn check_operators(graph: &GraphProto, allowed: &[&str]) -> Result<(), OnnxImportError> {
    for node in &graph.node {
        let op = node.op_type.as_str();
        if op.is_empty() {
            return Err(OnnxImportError::MalformedModel(format!(
                "node '{}' has empty op_type",
                node.name
            )));
        }
        if !allowed.contains(&op) {
            return Err(OnnxImportError::UnsupportedOperator {
                op_type: op.to_string(),
            });
        }
    }
    Ok(())
}

/// Derive a short architecture label from the operator mix for error messages.
pub fn detect_architecture(model: &ModelProto) -> String {
    let Some(graph) = model.graph.as_ref() else {
        return "unknown".into();
    };
    let mut has_tree = false;
    let mut has_linear = false;
    let mut has_matmul = false;
    let mut has_add = false;
    let mut has_relu = false;

    for node in &graph.node {
        match node.op_type.as_str() {
            "TreeEnsembleClassifier" => has_tree = true,
            "LinearClassifier" => has_linear = true,
            "MatMul" => has_matmul = true,
            "Add" => has_add = true,
            "Relu" => has_relu = true,
            _ => {}
        }
    }

    if has_tree {
        "decision tree (TreeEnsembleClassifier)".into()
    } else if has_linear {
        "logistic regression (LinearClassifier)".into()
    } else if has_matmul {
        let mut parts = vec!["MatMul"];
        if has_add {
            parts.push("Add");
        }
        if has_relu {
            parts.push("Relu");
        }
        format!("tiny MLP ({})", parts.join(" + "))
    } else {
        "unknown".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onnx::proto::{NodeProto, OperatorSetIdProto};

    #[test]
    fn rejects_empty_opset_list() {
        let model = ModelProto::default();
        assert!(matches!(
            check_opset(&model, 17),
            Err(OnnxImportError::MalformedModel(_))
        ));
    }

    #[test]
    fn ignores_unrelated_domains_when_core_present() {
        let model = ModelProto {
            opset_import: vec![
                OperatorSetIdProto {
                    domain: "com.example".into(),
                    version: 1,
                },
                OperatorSetIdProto {
                    domain: String::new(),
                    version: 17,
                },
            ],
            ..Default::default()
        };
        assert!(check_opset(&model, 17).is_ok());
    }

    #[test]
    fn operator_check_reports_first_offender() {
        let graph = GraphProto {
            name: "g".into(),
            node: vec![
                NodeProto {
                    op_type: "MatMul".into(),
                    ..Default::default()
                },
                NodeProto {
                    op_type: "Softmax".into(),
                    ..Default::default()
                },
            ],
        };
        match check_operators(&graph, &["MatMul", "Add", "Relu"]).unwrap_err() {
            OnnxImportError::UnsupportedOperator { op_type } => {
                assert_eq!(op_type, "Softmax");
            }
            other => panic!("unexpected: {other}"),
        }
    }
}
