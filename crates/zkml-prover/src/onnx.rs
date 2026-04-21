//! ONNX model importer.
//!
//! Parses ONNX protobuf files and converts supported operators into
//! the internal model representations defined in `zkml-common`.
//!
//! # Supported ONNX Operators (planned)
//!
//! | Operator           | Target Model            |
//! |--------------------|-------------------------|
//! | TreeEnsembleClassifier | `DecisionTree`      |
//! | LinearClassifier   | `LogisticRegression`    |
//! | MatMul + Add + Relu| `TinyMLP`               |

// TODO: Implement ONNX import using `prost`-generated types.
//
// The import pipeline will:
// 1. Deserialize the ONNX protobuf into a graph representation.
// 2. Walk the operator graph and identify the model architecture.
// 3. Extract weights, biases, and tree structures.
// 4. Quantize all floating-point values into `FixedPoint`.
// 5. Return the appropriate `Model` variant.

/// Placeholder for the ONNX import entry point.
///
/// In a future iteration, this function will accept raw ONNX bytes
/// and return a quantized `Model`.
pub fn import_onnx(_onnx_bytes: &[u8]) -> Result<zkml_common::models::Model, String> {
    Err("ONNX import is not yet implemented".to_string())
}
