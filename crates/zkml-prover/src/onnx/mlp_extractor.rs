//! MLP extraction from ONNX Gemm / MatMul + Add + Relu sequences.
//!
//! A typical PyTorch-exported MLP is a chain of `Gemm` nodes (or `MatMul` +
//! `Add` pairs) interleaved with `Relu` activations. This module walks the
//! graph topologically, identifies dense-layer groups, decodes the weight and
//! bias initialiser tensors, and produces a [`TinyMLP`].
//!
//! Supported patterns (per hidden layer):
//! - `Gemm(X, W, B)` — fused affine transform
//! - `MatMul(X, W)` → `Add(…, B)` — two-node affine
//!
//! Activation between layers must be `Relu`. The final layer may omit the
//! activation (raw logits), following the project convention.

#![allow(clippy::manual_is_multiple_of)]

use std::collections::HashMap;

use super::error::OnnxImportError;
use super::proto::{GraphProto, NodeProto, TensorProto};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{DenseLayer, TinyMLP};

// ---------------------------------------------------------------------------
// Initialiser helpers
// ---------------------------------------------------------------------------

/// Build a name → TensorProto lookup from the graph's initialiser list.
fn build_initializer_map(graph: &GraphProto) -> HashMap<&str, &TensorProto> {
    graph
        .initializer
        .iter()
        .map(|t| (t.name.as_str(), t))
        .collect()
}

/// Decode float-valued data from a `TensorProto` into a Vec<f64>.
///
/// Handles both `raw_data` (little-endian f32 bytes) and the explicit
/// `float_data` field. Returns an error for unsupported data types.
fn tensor_to_f64(tensor: &TensorProto) -> Result<Vec<f64>, OnnxImportError> {
    // data_type == 1 => FLOAT (f32)
    if tensor.data_type != 1 {
        return Err(OnnxImportError::MalformedModel(format!(
            "initialiser '{}' has data_type {} (only FLOAT/1 is supported)",
            tensor.name, tensor.data_type
        )));
    }

    if !tensor.raw_data.is_empty() {
        if tensor.raw_data.len() % 4 != 0 {
            return Err(OnnxImportError::MalformedModel(format!(
                "initialiser '{}' raw_data length {} is not a multiple of 4",
                tensor.name,
                tensor.raw_data.len()
            )));
        }
        Ok(tensor
            .raw_data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64)
            .collect())
    } else if !tensor.float_data.is_empty() {
        Ok(tensor.float_data.iter().map(|&v| v as f64).collect())
    } else {
        Err(OnnxImportError::MalformedModel(format!(
            "initialiser '{}' has no data (neither raw_data nor float_data)",
            tensor.name
        )))
    }
}

/// Return the shape of a TensorProto as `(rows, cols)`.
///
/// Supports 1-D (bias) and 2-D (weight matrix) tensors.
fn tensor_shape(tensor: &TensorProto) -> Result<Vec<usize>, OnnxImportError> {
    if tensor.dims.is_empty() {
        return Err(OnnxImportError::MalformedModel(format!(
            "initialiser '{}' has no shape dimensions",
            tensor.name
        )));
    }
    tensor
        .dims
        .iter()
        .map(|&d| {
            if d <= 0 {
                Err(OnnxImportError::MalformedModel(format!(
                    "initialiser '{}' has non-positive dimension {}",
                    tensor.name, d
                )))
            } else {
                Ok(d as usize)
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Gemm attribute helpers
// ---------------------------------------------------------------------------

/// Get an i64 attribute from a node, returning a default if absent.
fn get_int_attr(node: &NodeProto, name: &str, default: i64) -> i64 {
    node.attribute
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.i)
        .unwrap_or(default)
}

/// Get a float attribute from a node, returning a default if absent.
fn get_float_attr(node: &NodeProto, name: &str, default: f32) -> f32 {
    node.attribute
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.f)
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Per-layer extraction
// ---------------------------------------------------------------------------

/// A raw (un-quantized) dense layer before FixedPoint conversion.
struct RawLayer {
    weights: Vec<f64>, // row-major [output_size × input_size]
    biases: Vec<f64>,
    input_size: usize,
    output_size: usize,
}

/// Extract a dense layer from a Gemm node.
///
/// Gemm computes `Y = alpha * A * B + beta * C` with optional transposes.
/// Typical PyTorch export: `transB = 1`, `alpha = 1`, `beta = 1`.
fn extract_gemm_layer(
    node: &NodeProto,
    inits: &HashMap<&str, &TensorProto>,
) -> Result<RawLayer, OnnxImportError> {
    if node.input.len() < 2 {
        return Err(OnnxImportError::MalformedModel(format!(
            "Gemm node '{}' has fewer than 2 inputs",
            node.name
        )));
    }

    let alpha = get_float_attr(node, "alpha", 1.0);
    let beta = get_float_attr(node, "beta", 1.0);
    let trans_a = get_int_attr(node, "transA", 0);
    let trans_b = get_int_attr(node, "transB", 0);

    // We only support alpha=1, beta=1, transA=0
    if (alpha - 1.0).abs() > 1e-6 || trans_a != 0 {
        return Err(OnnxImportError::MalformedModel(format!(
            "Gemm node '{}': only alpha=1 and transA=0 are supported (got alpha={}, transA={})",
            node.name, alpha, trans_a
        )));
    }

    // Weight tensor
    let w_name = &node.input[1];
    let w_tensor = inits.get(w_name.as_str()).ok_or_else(|| {
        OnnxImportError::MalformedModel(format!(
            "Gemm node '{}': weight input '{}' not found in initialisers",
            node.name, w_name
        ))
    })?;

    let w_data = tensor_to_f64(w_tensor)?;
    let w_shape = tensor_shape(w_tensor)?;

    if w_shape.len() != 2 {
        return Err(OnnxImportError::MalformedModel(format!(
            "Gemm node '{}': weight tensor must be 2-D (got {} dims)",
            node.name,
            w_shape.len()
        )));
    }

    // Determine input_size and output_size considering transB
    let (input_size, output_size, weights_row_major) = if trans_b != 0 {
        // W shape is [output_size, input_size] (transposed before multiply)
        // Row-major weights are already [output_size × input_size]
        (w_shape[1], w_shape[0], w_data)
    } else {
        // W shape is [input_size, output_size]
        // Need to transpose to get row-major [output_size × input_size]
        let (rows, cols) = (w_shape[0], w_shape[1]);
        let mut transposed = vec![0.0; rows * cols];
        for r in 0..rows {
            for c in 0..cols {
                transposed[c * rows + r] = w_data[r * cols + c];
            }
        }
        (rows, cols, transposed)
    };

    // Apply alpha to weights
    let weights: Vec<f64> = if (alpha - 1.0).abs() > 1e-6 {
        weights_row_major
            .iter()
            .map(|&w| w * alpha as f64)
            .collect()
    } else {
        weights_row_major
    };

    // Bias tensor (optional third input)
    let biases = if node.input.len() >= 3 && !node.input[2].is_empty() {
        let b_name = &node.input[2];
        let b_tensor = inits.get(b_name.as_str()).ok_or_else(|| {
            OnnxImportError::MalformedModel(format!(
                "Gemm node '{}': bias input '{}' not found in initialisers",
                node.name, b_name
            ))
        })?;
        let mut b_data = tensor_to_f64(b_tensor)?;
        if b_data.len() != output_size {
            return Err(OnnxImportError::MalformedModel(format!(
                "Gemm node '{}': bias length {} does not match output_size {}",
                node.name,
                b_data.len(),
                output_size
            )));
        }
        if (beta - 1.0).abs() > 1e-6 {
            b_data.iter_mut().for_each(|b| *b *= beta as f64);
        }
        b_data
    } else {
        vec![0.0; output_size]
    };

    Ok(RawLayer {
        weights,
        biases,
        input_size,
        output_size,
    })
}

/// Extract a dense layer from a MatMul + Add node pair.
///
/// MatMul computes `Y = A @ B`. The paired Add node provides the bias.
fn extract_matmul_add_layer(
    matmul_node: &NodeProto,
    add_node: Option<&NodeProto>,
    inits: &HashMap<&str, &TensorProto>,
) -> Result<RawLayer, OnnxImportError> {
    if matmul_node.input.len() < 2 {
        return Err(OnnxImportError::MalformedModel(format!(
            "MatMul node '{}' has fewer than 2 inputs",
            matmul_node.name
        )));
    }

    // Weight tensor — the second input to MatMul
    let w_name = &matmul_node.input[1];
    let w_tensor = inits.get(w_name.as_str()).ok_or_else(|| {
        OnnxImportError::MalformedModel(format!(
            "MatMul node '{}': weight input '{}' not found in initialisers",
            matmul_node.name, w_name
        ))
    })?;

    let w_data = tensor_to_f64(w_tensor)?;
    let w_shape = tensor_shape(w_tensor)?;

    if w_shape.len() != 2 {
        return Err(OnnxImportError::MalformedModel(format!(
            "MatMul node '{}': weight tensor must be 2-D (got {} dims)",
            matmul_node.name,
            w_shape.len()
        )));
    }

    // MatMul: Y = X @ W where W is [input_size, output_size]
    // Need to transpose to row-major [output_size × input_size]
    let (input_size, output_size) = (w_shape[0], w_shape[1]);
    let mut weights = vec![0.0; input_size * output_size];
    for r in 0..input_size {
        for c in 0..output_size {
            weights[c * input_size + r] = w_data[r * output_size + c];
        }
    }

    // Bias from the Add node
    let biases = if let Some(add) = add_node {
        // The bias is whichever Add input is an initialiser
        let b_tensor = add
            .input
            .iter()
            .filter_map(|name| inits.get(name.as_str()))
            .next()
            .ok_or_else(|| {
                OnnxImportError::MalformedModel(format!(
                    "Add node '{}': no initialiser input found for bias",
                    add.name
                ))
            })?;
        let b_data = tensor_to_f64(b_tensor)?;
        if b_data.len() != output_size {
            return Err(OnnxImportError::MalformedModel(format!(
                "Add node '{}': bias length {} does not match output_size {}",
                add.name,
                b_data.len(),
                output_size
            )));
        }
        b_data
    } else {
        vec![0.0; output_size]
    };

    Ok(RawLayer {
        weights,
        biases,
        input_size,
        output_size,
    })
}

// ---------------------------------------------------------------------------
// Top-level MLP extractor
// ---------------------------------------------------------------------------

/// Extract a [`TinyMLP`] from a multi-node ONNX graph.
///
/// The graph must contain a sequence of dense layers (Gemm or MatMul + Add)
/// optionally interleaved with Relu activations. The Relu between hidden
/// layers is **expected** (it is the only supported activation) and the final
/// layer may omit it.
///
/// # Errors
///
/// - Unsupported activations (anything other than Relu).
/// - Missing initialisers for weight / bias tensors.
/// - Shape mismatches between consecutive layers.
pub fn extract_mlp(graph: &GraphProto) -> Result<TinyMLP, OnnxImportError> {
    let inits = build_initializer_map(graph);
    let nodes = &graph.node;

    if nodes.is_empty() {
        return Err(OnnxImportError::MalformedModel(
            "MLP graph has no nodes".into(),
        ));
    }

    // Validate that all operators are from the supported MLP set
    for node in nodes {
        match node.op_type.as_str() {
            "Gemm" | "MatMul" | "Add" | "Relu" => {}
            other => {
                return Err(OnnxImportError::MalformedModel(format!(
                    "MLP graph contains unsupported operator '{}' \
                     (expected Gemm, MatMul, Add, or Relu)",
                    other
                )));
            }
        }
    }

    let mut raw_layers: Vec<RawLayer> = Vec::new();
    let mut i = 0;

    while i < nodes.len() {
        let node = &nodes[i];

        match node.op_type.as_str() {
            "Gemm" => {
                raw_layers.push(extract_gemm_layer(node, &inits)?);
                i += 1;
            }
            "MatMul" => {
                // Look ahead for an Add node
                let add_node = if i + 1 < nodes.len() && nodes[i + 1].op_type == "Add" {
                    Some(&nodes[i + 1])
                } else {
                    None
                };
                raw_layers.push(extract_matmul_add_layer(node, add_node, &inits)?);
                i += if add_node.is_some() { 2 } else { 1 };
            }
            "Relu" => {
                // Relu is expected between hidden layers; skip it
                i += 1;
            }
            "Add" => {
                // Standalone Add without preceding MatMul — unexpected
                return Err(OnnxImportError::MalformedModel(format!(
                    "unexpected standalone Add node '{}' without preceding MatMul",
                    node.name
                )));
            }
            _ => {
                return Err(OnnxImportError::MalformedModel(format!(
                    "unexpected operator '{}' in MLP graph",
                    node.op_type
                )));
            }
        }
    }

    if raw_layers.is_empty() {
        return Err(OnnxImportError::MalformedModel(
            "MLP graph contains no dense layers (expected Gemm or MatMul + Add nodes)".into(),
        ));
    }

    // Validate layer chaining
    for j in 0..raw_layers.len().saturating_sub(1) {
        let out = raw_layers[j].output_size;
        let next_in = raw_layers[j + 1].input_size;
        if out != next_in {
            return Err(OnnxImportError::MalformedModel(format!(
                "layer {} output_size {} does not match layer {} input_size {}",
                j,
                out,
                j + 1,
                next_in
            )));
        }
    }

    // Quantize to FixedPoint
    let layers: Vec<DenseLayer> = raw_layers
        .into_iter()
        .map(|rl| DenseLayer {
            weights: rl
                .weights
                .iter()
                .map(|&w| FixedPoint::quantize(w))
                .collect(),
            biases: rl.biases.iter().map(|&b| FixedPoint::quantize(b)).collect(),
            input_size: rl.input_size,
            output_size: rl.output_size,
        })
        .collect();

    let mlp = TinyMLP { layers };
    mlp.validate()
        .map_err(|e| OnnxImportError::MalformedModel(format!("MLP validation failed: {e}")))?;

    Ok(mlp)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onnx::proto::{GraphProto, NodeProto, TensorProto};

    /// Helper to make a Gemm node with given input names.
    fn gemm_node(name: &str, inputs: Vec<&str>, trans_b: i64) -> NodeProto {
        use crate::onnx::proto::AttributeProto;
        NodeProto {
            name: name.into(),
            op_type: "Gemm".into(),
            domain: String::new(),
            input: inputs.into_iter().map(String::from).collect(),
            output: vec![format!("{name}_out")],
            attribute: vec![AttributeProto {
                name: "transB".into(),
                i: trans_b,
                ..Default::default()
            }],
        }
    }

    fn relu_node(name: &str) -> NodeProto {
        NodeProto {
            name: name.into(),
            op_type: "Relu".into(),
            domain: String::new(),
            input: vec![],
            output: vec![],
            attribute: vec![],
        }
    }

    /// Make a float TensorProto with given dims and float_data.
    fn make_tensor(name: &str, dims: Vec<i64>, data: Vec<f32>) -> TensorProto {
        TensorProto {
            name: name.into(),
            data_type: 1, // FLOAT
            dims,
            float_data: data,
            raw_data: vec![],
            int32_data: vec![],
            int64_data: vec![],
            string_data: vec![],
        }
    }

    #[test]
    fn single_gemm_layer_extracts() {
        // 2→1 layer: W = [[0.5], [-0.3]] shape [2,1] with transB=0
        // After transposing to row-major [1×2]: [0.5, -0.3]
        // But with transB=1 and W shape [1,2]: W is already [out×in]
        let w = make_tensor("W", vec![1, 2], vec![0.5, -0.3]);
        let b = make_tensor("B", vec![1], vec![0.1]);

        let graph = GraphProto {
            name: "test".into(),
            input: vec![],
            output: vec![],
            initializer: vec![w, b],
            node: vec![gemm_node("gemm0", vec!["X", "W", "B"], 1)],
        };

        let mlp = extract_mlp(&graph).unwrap();
        assert_eq!(mlp.layers.len(), 1);
        assert_eq!(mlp.layers[0].input_size, 2);
        assert_eq!(mlp.layers[0].output_size, 1);
    }

    #[test]
    fn two_layer_gemm_with_relu() {
        // Layer 1: 2→2 (identity), Layer 2: 2→1
        let w1 = make_tensor("W1", vec![2, 2], vec![1.0, 0.0, 0.0, 1.0]);
        let b1 = make_tensor("B1", vec![2], vec![0.0, 0.0]);
        let w2 = make_tensor("W2", vec![1, 2], vec![0.5, 1.0]);
        let b2 = make_tensor("B2", vec![1], vec![0.25]);

        let graph = GraphProto {
            name: "test".into(),
            input: vec![],
            output: vec![],
            initializer: vec![w1, b1, w2, b2],
            node: vec![
                gemm_node("gemm0", vec!["X", "W1", "B1"], 1),
                relu_node("relu0"),
                gemm_node("gemm1", vec!["relu_out", "W2", "B2"], 1),
            ],
        };

        let mlp = extract_mlp(&graph).unwrap();
        assert_eq!(mlp.layers.len(), 2);
        assert_eq!(mlp.layers[0].input_size, 2);
        assert_eq!(mlp.layers[0].output_size, 2);
        assert_eq!(mlp.layers[1].input_size, 2);
        assert_eq!(mlp.layers[1].output_size, 1);
    }

    #[test]
    fn missing_initialiser_fails() {
        let graph = GraphProto {
            name: "test".into(),
            input: vec![],
            output: vec![],
            initializer: vec![],
            node: vec![gemm_node("gemm0", vec!["X", "W", "B"], 1)],
        };

        let err = extract_mlp(&graph).unwrap_err();
        assert!(matches!(err, OnnxImportError::MalformedModel(msg) if msg.contains("not found")));
    }

    #[test]
    fn unsupported_op_in_mlp_graph_fails() {
        let graph = GraphProto {
            name: "test".into(),
            input: vec![],
            output: vec![],
            initializer: vec![],
            node: vec![NodeProto {
                name: "softmax".into(),
                op_type: "Softmax".into(),
                domain: String::new(),
                input: vec![],
                output: vec![],
                attribute: vec![],
            }],
        };

        let err = extract_mlp(&graph).unwrap_err();
        assert!(matches!(err, OnnxImportError::MalformedModel(msg) if msg.contains("Softmax")));
    }

    #[test]
    fn layer_chain_mismatch_fails() {
        // Layer 1: 2→3, Layer 2: 2→1 (input_size mismatch: 3 ≠ 2)
        let w1 = make_tensor("W1", vec![3, 2], vec![1.0; 6]);
        let b1 = make_tensor("B1", vec![3], vec![0.0; 3]);
        let w2 = make_tensor("W2", vec![1, 2], vec![0.5, 1.0]);
        let b2 = make_tensor("B2", vec![1], vec![0.0]);

        let graph = GraphProto {
            name: "test".into(),
            input: vec![],
            output: vec![],
            initializer: vec![w1, b1, w2, b2],
            node: vec![
                gemm_node("gemm0", vec!["X", "W1", "B1"], 1),
                relu_node("relu0"),
                gemm_node("gemm1", vec!["relu_out", "W2", "B2"], 1),
            ],
        };

        let err = extract_mlp(&graph).unwrap_err();
        assert!(
            matches!(err, OnnxImportError::MalformedModel(msg) if msg.contains("does not match"))
        );
    }

    #[test]
    fn matmul_add_pair_extracts() {
        // MatMul: X @ W where W is [2, 1], then Add bias
        let w = make_tensor("W", vec![2, 1], vec![0.5, -0.3]);
        let b = make_tensor("B", vec![1], vec![0.1]);

        let graph = GraphProto {
            name: "test".into(),
            input: vec![],
            output: vec![],
            initializer: vec![w, b],
            node: vec![
                NodeProto {
                    name: "matmul0".into(),
                    op_type: "MatMul".into(),
                    domain: String::new(),
                    input: vec!["X".into(), "W".into()],
                    output: vec!["mm_out".into()],
                    attribute: vec![],
                },
                NodeProto {
                    name: "add0".into(),
                    op_type: "Add".into(),
                    domain: String::new(),
                    input: vec!["mm_out".into(), "B".into()],
                    output: vec!["out".into()],
                    attribute: vec![],
                },
            ],
        };

        let mlp = extract_mlp(&graph).unwrap();
        assert_eq!(mlp.layers.len(), 1);
        assert_eq!(mlp.layers[0].input_size, 2);
        assert_eq!(mlp.layers[0].output_size, 1);
    }

    #[test]
    fn raw_data_tensor_decodes() {
        let val: f32 = 0.5;
        let raw = val.to_le_bytes().to_vec();
        let tensor = TensorProto {
            name: "t".into(),
            data_type: 1,
            dims: vec![1],
            raw_data: raw,
            float_data: vec![],
            int32_data: vec![],
            int64_data: vec![],
            string_data: vec![],
        };
        let data = tensor_to_f64(&tensor).unwrap();
        assert_eq!(data.len(), 1);
        assert!((data[0] - 0.5).abs() < 1e-6);
    }
}
