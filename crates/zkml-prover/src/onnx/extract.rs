//! Parameter extraction from ONNX operators into internal model types.

use super::error::OnnxImportError;
use super::proto::NodeProto;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::LogisticRegression;

/// Extract a binary logistic regression model from a LinearClassifier node.
///
/// # Arguments
///
/// * `node` - The LinearClassifier node to extract from
///
/// # Errors
///
/// - Returns an error if the model is multi-class (more than 2 classes)
/// - Returns an error if required attributes (coefficients, intercepts) are missing
/// - Returns an error if post_transform is not NONE or LOGISTIC
pub fn extract_linear_classifier(node: &NodeProto) -> Result<LogisticRegression, OnnxImportError> {
    // Extract coefficients (weights)
    let coefficients = get_floats_attribute(node, "coefficients").ok_or_else(|| {
        OnnxImportError::MalformedModel("LinearClassifier missing 'coefficients' attribute".into())
    })?;

    // Extract intercepts (bias)
    let intercepts = get_floats_attribute(node, "intercepts").ok_or_else(|| {
        OnnxImportError::MalformedModel("LinearClassifier missing 'intercepts' attribute".into())
    })?;

    // Check for multi-class (more than 1 coefficient vector)
    // For binary classification, skl2onnx stores coefficients as [num_features]
    // For multi-class, it stores as [num_classes * num_features]

    // Check if this is multi-class by looking at classlabels attributes
    let classlabels_ints = get_ints_attribute(node, "classlabels_ints");
    let classlabels_strings = get_strings_attribute(node, "classlabels_strings")?;

    let num_classes = if let Some(labels) = classlabels_ints {
        labels.len()
    } else if let Some(labels) = classlabels_strings {
        labels.len()
    } else {
        // If no classlabels, infer from intercepts length
        intercepts.len()
    };

    // Reject multi-class models
    if num_classes > 2 {
        return Err(OnnxImportError::MalformedModel(format!(
            "Multi-class LinearClassifier ({} classes) is not supported yet. Binary classification only. See issue #6 for multi-class support.",
            num_classes
        )));
    }

    // Validate intercepts length (should be 1 for binary classification)
    if intercepts.len() != 1 {
        return Err(OnnxImportError::MalformedModel(format!(
            "Binary LinearClassifier should have exactly 1 intercept, found {}",
            intercepts.len()
        )));
    }

    // Check post_transform attribute
    if let Some(post_transform) = get_string_attribute(node, "post_transform")? {
        if post_transform != "NONE" && post_transform != "LOGISTIC" {
            return Err(OnnxImportError::MalformedModel(format!(
                "Unsupported post_transform '{}'. Only NONE and LOGISTIC are supported (LOGISTIC is dropped in favor of thresholding the raw score).",
                post_transform
            )));
        }
        // Note: LOGISTIC is intentionally dropped because thresholding the raw score
        // is equivalent for binary decisions and sigmoid is not ZK-friendly
    }

    // Convert to f64 for quantization
    let weights_f64: Vec<f64> = coefficients.to_vec();
    let bias_f64 = intercepts[0];

    // Quantize using existing helpers
    let quantized_weights = weights_f64
        .iter()
        .map(|&w| FixedPoint::quantize(w))
        .collect();
    let quantized_bias = FixedPoint::quantize(bias_f64);

    Ok(LogisticRegression {
        weights: quantized_weights,
        bias: quantized_bias,
        decision_threshold: FixedPoint::quantize(0.0),
    })
}

/// Helper to get a list of floats from an attribute.
pub(crate) fn get_floats_attribute(node: &NodeProto, name: &str) -> Option<Vec<f64>> {
    node.attribute
        .iter()
        .find(|attr| attr.name == name)
        .and_then(|attr| {
            if !attr.floats.is_empty() {
                Some(attr.floats.iter().map(|&x| x as f64).collect())
            } else {
                None
            }
        })
}

/// Helper to get a list of ints from an attribute.
fn get_ints_attribute(node: &NodeProto, name: &str) -> Option<Vec<i64>> {
    node.attribute
        .iter()
        .find(|attr| attr.name == name)
        .and_then(|attr| {
            if !attr.ints.is_empty() {
                Some(attr.ints.clone())
            } else {
                None
            }
        })
}

/// Decode one attribute string, which the ONNX schema stores as bytes.
///
/// Invalid UTF-8 is an error rather than a skipped entry: the number of
/// `classlabels_strings` is what decides whether a model counts as
/// multi-class, so a dropped label could carry a rejected model past that gate.
fn decode_attribute_string(bytes: &[u8], attribute: &str) -> Result<String, OnnxImportError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        OnnxImportError::MalformedModel(format!(
            "attribute '{attribute}' holds a string that is not valid UTF-8"
        ))
    })
}

/// Helper to get a list of strings from an attribute.
pub(crate) fn get_strings_attribute(
    node: &NodeProto,
    name: &str,
) -> Result<Option<Vec<String>>, OnnxImportError> {
    let Some(attr) = node.attribute.iter().find(|attr| attr.name == name) else {
        return Ok(None);
    };

    if !attr.strings.is_empty() {
        let mut decoded = Vec::with_capacity(attr.strings.len());
        for bytes in &attr.strings {
            decoded.push(decode_attribute_string(bytes, name)?);
        }
        return Ok(Some(decoded));
    }

    if !attr.s.is_empty() {
        // Single value stored in 's' field
        return Ok(Some(vec![decode_attribute_string(&attr.s, name)?]));
    }

    Ok(None)
}

/// Helper to get a single string from an attribute.
fn get_string_attribute(node: &NodeProto, name: &str) -> Result<Option<String>, OnnxImportError> {
    Ok(get_strings_attribute(node, name)?.and_then(|mut values| {
        if values.is_empty() {
            None
        } else {
            Some(values.remove(0))
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onnx::proto::AttributeProto;

    fn make_node_with_coefficients(coeffs: Vec<f32>, intercepts: Vec<f32>) -> NodeProto {
        NodeProto {
            name: "test".into(),
            op_type: "LinearClassifier".into(),
            domain: "ai.onnx.ml".into(),
            input: vec!["X".into()],
            output: vec!["Y".into()],
            attribute: vec![
                AttributeProto {
                    name: "coefficients".into(),
                    floats: coeffs.to_vec(),
                    f: 0.0,
                    i: 0,
                    ints: vec![],
                    s: vec![],
                    strings: vec![],
                    t: None,
                    g: None,
                    sparse_tensor: None,
                    r#type: 0,
                },
                AttributeProto {
                    name: "intercepts".into(),
                    floats: intercepts.to_vec(),
                    f: 0.0,
                    i: 0,
                    ints: vec![],
                    s: vec![],
                    strings: vec![],
                    t: None,
                    g: None,
                    sparse_tensor: None,
                    r#type: 0,
                },
            ],
        }
    }

    /// Three class labels, one of them invalid UTF-8.
    ///
    /// Dropping the bad entry would leave `num_classes` at 2 and let a
    /// three-class model through the gate that rejects multi-class.
    #[test]
    fn a_label_that_is_not_utf8_is_an_error_not_a_dropped_entry() {
        let mut node = make_node_with_coefficients(vec![1.0, 2.0, 3.0], vec![0.1, 0.2, 0.3]);
        node.attribute.push(AttributeProto {
            name: "classlabels_strings".into(),
            strings: vec![b"low".to_vec(), vec![0xff, 0xfe], b"high".to_vec()],
            floats: vec![],
            f: 0.0,
            i: 0,
            ints: vec![],
            s: vec![],
            t: None,
            g: None,
            sparse_tensor: None,
            r#type: 0,
        });

        let err = extract_linear_classifier(&node).unwrap_err();

        match err {
            OnnxImportError::MalformedModel(message) => {
                assert!(
                    message.contains("not valid UTF-8"),
                    "expected the decode to be reported, got: {message}"
                );
            }
            other => panic!("expected MalformedModel, got {other:?}"),
        }
    }

    #[test]
    fn extract_binary_classifier_succeeds() {
        let node = make_node_with_coefficients(vec![0.5, -0.3, 0.8], vec![0.1]);
        let result = extract_linear_classifier(&node);
        assert!(result.is_ok());
        let lr = result.unwrap();
        assert_eq!(lr.weights.len(), 3);
    }

    #[test]
    fn multi_class_rejected() {
        let mut node = make_node_with_coefficients(vec![0.5, -0.3, 0.8], vec![0.1, 0.2]);
        node.attribute.push(AttributeProto {
            name: "classlabels_ints".into(),
            floats: vec![],
            f: 0.0,
            i: 0,
            ints: vec![0, 1, 2], // 3 classes
            s: vec![],
            strings: vec![],
            t: None,
            g: None,
            sparse_tensor: None,
            r#type: 0,
        });
        let result = extract_linear_classifier(&node);
        assert!(result.is_err());
        match result.unwrap_err() {
            OnnxImportError::MalformedModel(msg) => {
                assert!(msg.contains("Multi-class"));
                assert!(msg.contains("3 classes"));
            }
            _ => panic!("Expected MalformedModel error"),
        }
    }
}
