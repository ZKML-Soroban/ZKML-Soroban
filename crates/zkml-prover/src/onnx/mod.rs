//! ONNX model importer foundation.
//!
//! This module deserializes ONNX protobuf (`ModelProto`), validates the opset
//! version and operator set, and returns a typed error. Parameter extraction
//! into internal model types is intentionally deferred:
//!
//! - Tree ensemble extraction: GitHub issue #5
//! - Linear classifier extraction: GitHub issue #6
//!
//! # Supported operators (allowlist)
//!
//! | Operator                | Target (future)        |
//! |-------------------------|------------------------|
//! | `TreeEnsembleClassifier`| `DecisionTree`         |
//! | `LinearClassifier`      | `LogisticRegression`   |
//! | `MatMul`                | `TinyMLP` dense layer  |
//! | `Add`                   | `TinyMLP` bias         |
//! | `Relu`                  | `TinyMLP` activation   |
//!
//! Unsupported operators fail at import time with a clear error rather than
//! silently ignoring nodes.

mod error;
mod proto;
mod validate;

pub use error::OnnxImportError;
pub use proto::{GraphProto, ModelProto, NodeProto, OperatorSetIdProto};

use prost::Message;
use validate::{check_operators, check_opset, detect_architecture};
use zkml_common::models::Model;

/// Minimum ONNX opset version accepted by this importer.
///
/// Matches the project compatibility target documented in
/// `docs/technical-overview.md` (ONNX Compatibility).
pub const MIN_OPSET_VERSION: i64 = 17;

/// Operators currently allowed by the importer foundation.
///
/// Extraction of each family is tracked in issues #5 (trees) and #6 (linear).
pub const SUPPORTED_OPERATORS: &[&str] = &[
    "TreeEnsembleClassifier",
    "LinearClassifier",
    "MatMul",
    "Add",
    "Relu",
];

/// Import an ONNX model from a protobuf byte slice.
///
/// Performs protobuf decoding, opset validation (`>= 17`), and operator
/// allowlist checks. When validation succeeds, returns
/// [`OnnxImportError::ExtractionNotImplemented`] until parameter extraction
/// lands in issues #5 / #6.
///
/// # Errors
///
/// - [`OnnxImportError::MalformedModel`] if the bytes are not a valid
///   `ModelProto` or the graph is missing / empty.
/// - [`OnnxImportError::UnsupportedOpset`] if any relevant opset is below 17.
/// - [`OnnxImportError::UnsupportedOperator`] if a graph node uses an op
///   outside the allowlist.
/// - [`OnnxImportError::ExtractionNotImplemented`] after successful validation.
pub fn import_onnx(bytes: &[u8]) -> Result<Model, OnnxImportError> {
    let model = parse_model_proto(bytes)?;
    validate_model(&model)?;
    let hint = detect_architecture(&model);
    Err(OnnxImportError::ExtractionNotImplemented {
        architecture_hint: hint,
    })
}

/// Decode raw bytes into an ONNX `ModelProto` without further validation.
///
/// Useful for tests and for callers that want to inspect the graph after
/// [`import_onnx`] rejects extraction.
pub fn parse_model_proto(bytes: &[u8]) -> Result<ModelProto, OnnxImportError> {
    ModelProto::decode(bytes)
        .map_err(|e| OnnxImportError::MalformedModel(format!("protobuf decode failed: {e}")))
}

/// Validate opset version and operator allowlist on an already-decoded model.
pub fn validate_model(model: &ModelProto) -> Result<(), OnnxImportError> {
    check_opset(model, MIN_OPSET_VERSION)?;
    let graph = model
        .graph
        .as_ref()
        .ok_or_else(|| OnnxImportError::MalformedModel("model has no graph".into()))?;
    if graph.node.is_empty() {
        return Err(OnnxImportError::MalformedModel(
            "model graph has no nodes".into(),
        ));
    }
    check_operators(graph, SUPPORTED_OPERATORS)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn encode(model: &ModelProto) -> Vec<u8> {
        let mut buf = Vec::new();
        model.encode(&mut buf).expect("encode model");
        buf
    }

    fn model_with(opset: i64, ops: &[&str]) -> ModelProto {
        ModelProto {
            ir_version: 8,
            opset_import: vec![
                OperatorSetIdProto {
                    domain: String::new(),
                    version: opset,
                },
                OperatorSetIdProto {
                    domain: "ai.onnx.ml".into(),
                    version: opset,
                },
            ],
            graph: Some(GraphProto {
                name: "test".into(),
                node: ops
                    .iter()
                    .enumerate()
                    .map(|(i, op)| NodeProto {
                        name: format!("n{i}"),
                        op_type: (*op).into(),
                        domain: if op.ends_with("Classifier") {
                            "ai.onnx.ml".into()
                        } else {
                            String::new()
                        },
                        input: vec!["X".into()],
                        output: vec![format!("Y{i}")],
                    })
                    .collect(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn valid_tree_reaches_extraction_not_implemented() {
        let bytes = encode(&model_with(17, &["TreeEnsembleClassifier"]));
        let err = import_onnx(&bytes).unwrap_err();
        match err {
            OnnxImportError::ExtractionNotImplemented { architecture_hint } => {
                assert!(
                    architecture_hint.to_lowercase().contains("tree"),
                    "hint was: {architecture_hint}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn valid_linear_reaches_extraction_not_implemented() {
        let bytes = encode(&model_with(18, &["LinearClassifier"]));
        let err = import_onnx(&bytes).unwrap_err();
        assert!(matches!(
            err,
            OnnxImportError::ExtractionNotImplemented { .. }
        ));
    }

    #[test]
    fn valid_mlp_ops_reaches_extraction_not_implemented() {
        let bytes = encode(&model_with(17, &["MatMul", "Add", "Relu"]));
        let err = import_onnx(&bytes).unwrap_err();
        assert!(matches!(
            err,
            OnnxImportError::ExtractionNotImplemented { .. }
        ));
    }

    #[test]
    fn unsupported_operator_names_offender() {
        let bytes = encode(&model_with(17, &["Conv"]));
        let err = import_onnx(&bytes).unwrap_err();
        match err {
            OnnxImportError::UnsupportedOperator { op_type } => assert_eq!(op_type, "Conv"),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn low_opset_is_rejected() {
        let bytes = encode(&model_with(13, &["TreeEnsembleClassifier"]));
        let err = import_onnx(&bytes).unwrap_err();
        match err {
            OnnxImportError::UnsupportedOpset { found, required } => {
                assert_eq!(found, 13);
                assert_eq!(required, MIN_OPSET_VERSION);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn garbage_bytes_are_malformed() {
        let err = import_onnx(b"not a protobuf model").unwrap_err();
        assert!(matches!(err, OnnxImportError::MalformedModel(_)));
    }

    #[test]
    fn missing_graph_is_malformed() {
        let model = ModelProto {
            ir_version: 8,
            opset_import: vec![OperatorSetIdProto {
                domain: String::new(),
                version: 17,
            }],
            graph: None,
            ..Default::default()
        };
        let err = import_onnx(&encode(&model)).unwrap_err();
        assert!(matches!(err, OnnxImportError::MalformedModel(_)));
    }

    #[test]
    fn empty_graph_is_malformed() {
        let model = ModelProto {
            ir_version: 8,
            opset_import: vec![OperatorSetIdProto {
                domain: String::new(),
                version: 17,
            }],
            graph: Some(GraphProto {
                name: "empty".into(),
                node: vec![],
            }),
            ..Default::default()
        };
        let err = import_onnx(&encode(&model)).unwrap_err();
        assert!(matches!(err, OnnxImportError::MalformedModel(_)));
    }

    #[test]
    fn parse_model_proto_round_trips() {
        let original = model_with(17, &["Add", "Relu"]);
        let bytes = encode(&original);
        let decoded = parse_model_proto(&bytes).unwrap();
        assert_eq!(decoded.opset_import[0].version, 17);
        assert_eq!(decoded.graph.as_ref().unwrap().node.len(), 2);
    }
}
