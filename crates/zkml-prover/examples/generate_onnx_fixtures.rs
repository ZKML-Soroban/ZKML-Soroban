//! One-shot helper that writes the ONNX fixture files used by import tests.
//!
//! ```text
//! cargo run -p zkml-prover --example generate_onnx_fixtures
//! ```
//!
//! Fixtures are synthetic `ModelProto` encodings (same field tags as real ONNX
//! files). A production-style decision-tree export from skl2onnx is documented
//! in `tests/fixtures/README.md` for when Python tooling is available; the
//! committed binaries below already exercise every validation path.

use std::fs;
use std::path::PathBuf;

use prost::Message;
use zkml_prover::onnx::{GraphProto, ModelProto, NodeProto, OperatorSetIdProto};

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
    }
}

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::create_dir_all(&out).expect("create fixtures dir");

    // Valid decision-tree shaped model (opset 17, TreeEnsembleClassifier only).
    let tree = ModelProto {
        ir_version: 8,
        producer_name: "zkml-fixture-generator".into(),
        producer_version: "0.1".into(),
        opset_import: vec![
            OperatorSetIdProto {
                domain: String::new(),
                version: 17,
            },
            OperatorSetIdProto {
                domain: "ai.onnx.ml".into(),
                version: 17,
            },
        ],
        graph: Some(GraphProto {
            name: "decision_tree".into(),
            node: vec![node(
                "tree_ensemble",
                "TreeEnsembleClassifier",
                "ai.onnx.ml",
            )],
        }),
        ..Default::default()
    };
    fs::write(out.join("decision_tree_valid.onnx"), encode(&tree)).unwrap();

    // Intentionally unsupported operator (Conv) with a valid opset.
    let unsupported = ModelProto {
        ir_version: 8,
        producer_name: "zkml-fixture-generator".into(),
        opset_import: vec![OperatorSetIdProto {
            domain: String::new(),
            version: 17,
        }],
        graph: Some(GraphProto {
            name: "cnn".into(),
            node: vec![node("conv0", "Conv", "")],
        }),
        ..Default::default()
    };
    fs::write(out.join("unsupported_conv.onnx"), encode(&unsupported)).unwrap();

    // Valid operators but opset below the required minimum of 17.
    let low_opset = ModelProto {
        ir_version: 7,
        producer_name: "zkml-fixture-generator".into(),
        opset_import: vec![
            OperatorSetIdProto {
                domain: String::new(),
                version: 13,
            },
            OperatorSetIdProto {
                domain: "ai.onnx.ml".into(),
                version: 13,
            },
        ],
        graph: Some(GraphProto {
            name: "old_tree".into(),
            node: vec![node(
                "tree_ensemble",
                "TreeEnsembleClassifier",
                "ai.onnx.ml",
            )],
        }),
        ..Default::default()
    };
    fs::write(out.join("low_opset_tree.onnx"), encode(&low_opset)).unwrap();

    // Linear classifier with supported opset (validation OK, extraction pending).
    let linear = ModelProto {
        ir_version: 8,
        producer_name: "zkml-fixture-generator".into(),
        opset_import: vec![
            OperatorSetIdProto {
                domain: String::new(),
                version: 18,
            },
            OperatorSetIdProto {
                domain: "ai.onnx.ml".into(),
                version: 18,
            },
        ],
        graph: Some(GraphProto {
            name: "logistic".into(),
            node: vec![node("linear", "LinearClassifier", "ai.onnx.ml")],
        }),
        ..Default::default()
    };
    fs::write(out.join("linear_classifier_valid.onnx"), encode(&linear)).unwrap();

    println!("wrote fixtures to {}", out.display());
}
