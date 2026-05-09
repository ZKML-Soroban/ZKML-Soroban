//! Model importer.
//!
//! Native ONNX protobuf parsing is still pending. In the meantime the import
//! entrypoint accepts the JSON exchange format documented in
//! `docs/model-format.md` and lowers it into the internal `Model` type, so the
//! rest of the pipeline can be built and tested end to end.
//!
//! # Planned ONNX operator mapping
//!
//! | Operator                | Target Model           |
//! |-------------------------|------------------------|
//! | TreeEnsembleClassifier  | `DecisionTree`         |
//! | LinearClassifier        | `LogisticRegression`   |
//! | MatMul + Add + Relu     | `TinyMLP`              |

use zkml_common::models::Model;

/// Import a model from the JSON exchange format.
///
/// # Errors
///
/// Returns a descriptive error string if the bytes cannot be parsed.
pub fn import_onnx(bytes: &[u8]) -> Result<Model, String> {
    let doc: crate::model_io::JsonModel =
        serde_json::from_slice(bytes).map_err(|e| format!("model parse error: {e}"))?;
    Ok(doc.into_model())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_rejects_garbage() {
        assert!(import_onnx(b"not json").is_err());
    }
}
