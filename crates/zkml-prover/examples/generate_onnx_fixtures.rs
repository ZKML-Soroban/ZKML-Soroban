//! One-shot helper that writes the ONNX fixture files used by import tests.
//!
//! ```text
//! cargo run -p zkml-prover --example generate_onnx_fixtures
//! ```
//!
//! Fixtures are synthetic `ModelProto` encodings (same field tags as real ONNX
//! files). Opset pairs mirror real exporters: core `ai.onnx` >= 17 and
//! `ai.onnx.ml` in 1–5 (never ml=17).

use std::fs;
use std::path::PathBuf;

use prost::Message;
use zkml_prover::onnx::{
    AttributeProto, GraphProto, ModelProto, NodeProto, OperatorSetIdProto, TensorProto,
};

fn encode(model: &ModelProto) -> Vec<u8> {
    let mut buf = Vec::new();
    model.encode(&mut buf).expect("encode");
    buf
}

fn node(name: &str, op_type: &str, domain: &str) -> NodeProto {
    NodeProto {
        name: name.into(),
        op_type: op_type.into(),
        domain: domain.into(),
        input: vec!["X".into()],
        output: vec!["Y".into()],
        attribute: vec![],
    }
}

fn linear_classifier_node(name: &str, coefficients: Vec<f32>, intercepts: Vec<f32>) -> NodeProto {
    NodeProto {
        name: name.into(),
        op_type: "LinearClassifier".into(),
        domain: "ai.onnx.ml".into(),
        input: vec!["X".into()],
        output: vec!["Y".into()],
        attribute: vec![
            AttributeProto {
                name: "coefficients".into(),
                floats: coefficients.iter().map(|&x| x as f64).collect(),
                f: 0.0,
                i: 0,
                ints: vec![],
                s: vec![],
                strings: vec![],
                t: None,
                g: None,
                floats_f32: vec![],
                ints_extra: vec![],
                strings_bytes: vec![],
                sparse_tensor: None,
                r#type: 0,
            },
            AttributeProto {
                name: "intercepts".into(),
                floats: intercepts.iter().map(|&x| x as f64).collect(),
                f: 0.0,
                i: 0,
                ints: vec![],
                s: vec![],
                strings: vec![],
                t: None,
                g: None,
                floats_f32: vec![],
                ints_extra: vec![],
                strings_bytes: vec![],
                sparse_tensor: None,
                r#type: 0,
            },
            AttributeProto {
                name: "post_transform".into(),
                floats: vec![],
                f: 0.0,
                i: 0,
                ints: vec![],
                s: b"NONE".to_vec(),
                strings: vec![],
                t: None,
                g: None,
                floats_f32: vec![],
                ints_extra: vec![],
                strings_bytes: vec![],
                sparse_tensor: None,
                r#type: 0,
            },
        ],
    }
}

fn tree_classifier_node(name: &str) -> NodeProto {
    // Simple tree: if feature[0] <= 0.5 then leaf 0 else leaf 1
    // Node 0: split on feature 0, threshold 0.5
    // Node 1: leaf with value 0.0
    // Node 2: leaf with value 1.0
    NodeProto {
        name: name.into(),
        op_type: "TreeEnsembleClassifier".into(),
        domain: "ai.onnx.ml".into(),
        input: vec!["X".into()],
        output: vec!["Y".into()],
        attribute: vec![
            // Tree structure attributes
            AttributeProto {
                name: "nodes_treeids".into(),
                ints: vec![0, 0, 0], // All nodes in tree 0
                ..Default::default()
            },
            AttributeProto {
                name: "nodes_nodeids".into(),
                ints: vec![0, 1, 2], // Node IDs
                ..Default::default()
            },
            AttributeProto {
                name: "nodes_featureids".into(),
                ints: vec![0, 0, 0], // All on feature 0
                ..Default::default()
            },
            AttributeProto {
                name: "nodes_values".into(),
                floats: vec![0.5, 0.0, 0.0], // Thresholds (leaf values are 0 in ONNX)
                ..Default::default()
            },
            AttributeProto {
                name: "nodes_modes".into(),
                strings: vec!["BRANCH_LEQ".into(), "LEAF".into(), "LEAF".into()],
                ..Default::default()
            },
            AttributeProto {
                name: "nodes_truenodeids".into(),
                ints: vec![1, 0, 0], // Child node IDs (not indices)
                ..Default::default()
            },
            AttributeProto {
                name: "nodes_falsenodeids".into(),
                ints: vec![2, 0, 0], // Child node IDs (not indices)
                ..Default::default()
            },
            // Class attributes for leaf values
            AttributeProto {
                name: "class_ids".into(),
                ints: vec![0, 1], // Class indices
                ..Default::default()
            },
            AttributeProto {
                name: "class_weights".into(),
                floats: vec![0.0, 1.0], // Actual leaf values
                ..Default::default()
            },
            AttributeProto {
                name: "class_nodeids".into(),
                ints: vec![1, 2], // Which leaf nodes these weights belong to
                ..Default::default()
            },
            AttributeProto {
                name: "class_treeids".into(),
                ints: vec![0, 0], // All in tree 0
                ..Default::default()
            },
        ],
    }
}

fn opsets(core: i64, ml: i64) -> Vec<OperatorSetIdProto> {
    vec![
        OperatorSetIdProto {
            domain: String::new(),
            version: core,
        },
        OperatorSetIdProto {
            domain: "ai.onnx.ml".into(),
            version: ml,
        },
    ]
}

fn make_tensor(name: &str, dims: Vec<i64>, data: Vec<f32>) -> TensorProto {
    TensorProto {
        name: name.into(),
        data_type: 1, // FLOAT (f32)
        dims,
        float_data: data,
        raw_data: vec![],
        int32_data: vec![],
        int64_data: vec![],
        string_data: vec![],
    }
}

fn gemm_node(name: &str, inputs: Vec<&str>, output: &str, trans_b: i64) -> NodeProto {
    NodeProto {
        name: name.into(),
        op_type: "Gemm".into(),
        domain: String::new(),
        input: inputs.into_iter().map(String::from).collect(),
        output: vec![output.into()],
        attribute: vec![AttributeProto {
            name: "transB".into(),
            i: trans_b,
            ..Default::default()
        }],
    }
}

fn relu_node(name: &str, input: &str, output: &str) -> NodeProto {
    NodeProto {
        name: name.into(),
        op_type: "Relu".into(),
        domain: String::new(),
        input: vec![input.into()],
        output: vec![output.into()],
        attribute: vec![],
    }
}

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::create_dir_all(&out).expect("create fixtures dir");

    // Valid decision-tree: realistic skl2onnx-like pair (core 17 + ml 3).
    let tree = ModelProto {
        ir_version: 8,
        producer_name: "zkml-fixture-generator".into(),
        producer_version: "0.1".into(),
        opset_import: opsets(17, 3),
        graph: Some(GraphProto {
            name: "decision_tree".into(),
            input: vec![],
            output: vec![],
            initializer: vec![],
            node: vec![tree_classifier_node("tree_ensemble")],
        }),
        ..Default::default()
    };
    fs::write(out.join("decision_tree_valid.onnx"), encode(&tree)).unwrap();

    // Explicit skl2onnx-like layout (same graph; distinct name for regression).
    let skl = ModelProto {
        ir_version: 8,
        producer_name: "skl2onnx".into(),
        producer_version: "1.16".into(),
        opset_import: opsets(17, 3),
        graph: Some(GraphProto {
            name: "SklearnDecisionTreeClassifier".into(),
            input: vec![],
            output: vec![],
            initializer: vec![],
            node: vec![tree_classifier_node("TreeEnsembleClassifier")],
        }),
        ..Default::default()
    };
    fs::write(out.join("skl2onnx_like_tree.onnx"), encode(&skl)).unwrap();

    // Intentionally unsupported operator (Conv) with a valid core opset.
    let unsupported = ModelProto {
        ir_version: 8,
        producer_name: "zkml-fixture-generator".into(),
        opset_import: vec![OperatorSetIdProto {
            domain: String::new(),
            version: 17,
        }],
        graph: Some(GraphProto {
            name: "cnn".into(),
            input: vec![],
            output: vec![],
            initializer: vec![],
            node: vec![node("conv0", "Conv", "")],
        }),
        ..Default::default()
    };
    fs::write(out.join("unsupported_conv.onnx"), encode(&unsupported)).unwrap();

    // Core domain below floor (ml is fine at 3).
    let low_opset = ModelProto {
        ir_version: 7,
        producer_name: "zkml-fixture-generator".into(),
        opset_import: opsets(13, 3),
        graph: Some(GraphProto {
            name: "old_tree".into(),
            input: vec![],
            output: vec![],
            initializer: vec![],
            node: vec![tree_classifier_node("tree_ensemble")],
        }),
        ..Default::default()
    };
    fs::write(out.join("low_opset_tree.onnx"), encode(&low_opset)).unwrap();

    // Linear classifier: core 18 + ml 1 (LinearClassifier is ml opset 1 only).
    // Binary classifier with 3 features.
    let linear = ModelProto {
        ir_version: 8,
        producer_name: "zkml-fixture-generator".into(),
        opset_import: opsets(18, 1),
        graph: Some(GraphProto {
            name: "logistic".into(),
            input: vec![],
            output: vec![],
            initializer: vec![],
            node: vec![linear_classifier_node(
                "linear",
                vec![0.5, -0.3, 0.8], // 3 coefficients
                vec![0.1],            // 1 intercept (binary)
            )],
        }),
        ..Default::default()
    };
    fs::write(out.join("linear_classifier_valid.onnx"), encode(&linear)).unwrap();

    // TinyMLP: core 17 (no ML domain as it uses core ops Gemm/Relu).
    // 2->2->1 MLP matching the golden_network in tinymlp_inference.rs.
    let w1 = make_tensor("W1", vec![2, 2], vec![0.3, -0.2, 0.1, 0.4]);
    let b1 = make_tensor("B1", vec![2], vec![0.1, -0.5]);
    let w2 = make_tensor("W2", vec![1, 2], vec![0.6, 0.2]);
    let b2 = make_tensor("B2", vec![1], vec![0.05]);

    let mlp = ModelProto {
        ir_version: 8,
        producer_name: "zkml-fixture-generator".into(),
        producer_version: "0.1".into(),
        opset_import: vec![OperatorSetIdProto {
            domain: String::new(),
            version: 17,
        }],
        graph: Some(GraphProto {
            name: "tinymlp".into(),
            input: vec![],
            output: vec![],
            initializer: vec![w1, b1, w2, b2],
            node: vec![
                gemm_node("gemm0", vec!["X", "W1", "B1"], "gemm0_out", 1),
                relu_node("relu0", "gemm0_out", "relu0_out"),
                gemm_node("gemm1", vec!["relu0_out", "W2", "B2"], "Y", 1),
            ],
        }),
        ..Default::default()
    };
    fs::write(out.join("tinymlp_valid.onnx"), encode(&mlp)).unwrap();

    println!("wrote fixtures to {}", out.display());
}
