//! Typed errors for the ONNX import pipeline.

use core::fmt;

/// Errors produced while parsing or validating an ONNX model.
///
/// Unsupported operators and opset versions fail here at import time so
/// callers never proceed with a silently incomplete graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnnxImportError {
    /// Protobuf decoding failed or the model structure is incomplete.
    MalformedModel(String),
    /// An opset import is below the minimum supported version.
    UnsupportedOpset {
        /// The opset version found in the model.
        found: i64,
        /// The minimum version required by this importer.
        required: i64,
    },
    /// A graph node uses an operator outside the allowlist.
    UnsupportedOperator {
        /// The ONNX `op_type` string of the offending node.
        op_type: String,
    },
    /// Validation succeeded but parameter extraction is not implemented yet.
    ///
    /// Tree extraction: issue #5. Linear classifier extraction: issue #6.
    ExtractionNotImplemented {
        /// Best-effort architecture label derived from the operator set.
        architecture_hint: String,
    },
}

impl fmt::Display for OnnxImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OnnxImportError::MalformedModel(msg) => {
                write!(f, "malformed ONNX model: {msg}")
            }
            OnnxImportError::UnsupportedOpset { found, required } => {
                write!(f, "unsupported ONNX opset {found} (required >= {required})")
            }
            OnnxImportError::UnsupportedOperator { op_type } => {
                write!(
                    f,
                    "unsupported ONNX operator '{op_type}' (supported: TreeEnsembleClassifier LinearClassifier MatMul Add Relu)"
                )
            }
            OnnxImportError::ExtractionNotImplemented { architecture_hint } => {
                write!(
                    f,
                    "ONNX validation succeeded for {architecture_hint} but parameter extraction is not implemented yet (see issues #5 and #6)"
                )
            }
        }
    }
}

impl std::error::Error for OnnxImportError {}
