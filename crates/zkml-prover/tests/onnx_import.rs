//! Integration tests for the ONNX importer foundation using committed fixtures.

use zkml_prover::onnx::{
    import_onnx, parse_model_proto, OnnxImportError, MIN_OPSET_CORE, MIN_OPSET_VERSION,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {path}: {e}"))
}

#[test]
fn fixture_decision_tree_validates_then_defers_extraction() {
    let bytes = fixture("decision_tree_valid.onnx");
    let model = parse_model_proto(&bytes).expect("valid onnx decodes");
    assert!(model.graph.is_some());
    assert!(!model.graph.as_ref().unwrap().node.is_empty());
    // Realistic pair: core 17 + ai.onnx.ml 3.
    let ml = model
        .opset_import
        .iter()
        .find(|e| e.domain == "ai.onnx.ml")
        .expect("ml opset");
    assert_eq!(ml.version, 3);

    let err = import_onnx(&bytes).unwrap_err();
    match err {
        OnnxImportError::ExtractionNotImplemented { architecture_hint } => {
            assert!(
                architecture_hint.to_lowercase().contains("tree"),
                "unexpected hint: {architecture_hint}"
            );
        }
        other => panic!("expected ExtractionNotImplemented, got {other}"),
    }
}

#[test]
fn fixture_skl2onnx_like_tree_is_accepted() {
    let bytes = fixture("skl2onnx_like_tree.onnx");
    let model = parse_model_proto(&bytes).expect("skl2onnx-like onnx decodes");
    let core = model
        .opset_import
        .iter()
        .find(|e| e.domain.is_empty() || e.domain == "ai.onnx")
        .expect("core opset");
    let ml = model
        .opset_import
        .iter()
        .find(|e| e.domain == "ai.onnx.ml")
        .expect("ml opset");
    assert!(core.version >= MIN_OPSET_CORE);
    assert!(ml.version >= 1 && ml.version <= 5);

    let err = import_onnx(&bytes).unwrap_err();
    assert!(
        matches!(err, OnnxImportError::ExtractionNotImplemented { .. }),
        "got {err}"
    );
}

#[test]
fn fixture_linear_validates_then_defers_extraction() {
    let bytes = fixture("linear_classifier_valid.onnx");
    let model = parse_model_proto(&bytes).expect("linear onnx decodes");
    let ml = model
        .opset_import
        .iter()
        .find(|e| e.domain == "ai.onnx.ml")
        .expect("ml opset");
    assert_eq!(ml.version, 1);

    let err = import_onnx(&bytes).unwrap_err();
    assert!(
        matches!(err, OnnxImportError::ExtractionNotImplemented { .. }),
        "got {err}"
    );
}

#[test]
fn fixture_unsupported_conv_names_operator() {
    let bytes = fixture("unsupported_conv.onnx");
    let err = import_onnx(&bytes).unwrap_err();
    match &err {
        OnnxImportError::UnsupportedOperator { op_type } => {
            assert_eq!(op_type, "Conv");
            let msg = err.to_string();
            assert!(msg.contains("Conv"), "display was: {msg}");
        }
        other => panic!("expected UnsupportedOperator, got {other}"),
    }
}

#[test]
fn fixture_low_opset_is_rejected() {
    let bytes = fixture("low_opset_tree.onnx");
    let err = import_onnx(&bytes).unwrap_err();
    match err {
        OnnxImportError::UnsupportedOpset { found, required } => {
            assert_eq!(found, 13);
            assert_eq!(required, MIN_OPSET_VERSION);
            let msg = err.to_string();
            assert!(msg.contains("13"), "display was: {msg}");
            assert!(msg.contains("17"), "display was: {msg}");
        }
        other => panic!("expected UnsupportedOpset, got {other}"),
    }
}

#[test]
fn garbage_input_does_not_panic() {
    let err = import_onnx(b"\x00\x01\x02not-onnx").unwrap_err();
    assert!(matches!(err, OnnxImportError::MalformedModel(_)));
}
