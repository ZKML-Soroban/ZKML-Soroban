//! Acceptance tests for TinyMLP fixed-point inference (issue #7).

use zkml_common::error::ZkmlError;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{DenseLayer, Model, TinyMLP};
use zkml_prover::inference::{run_inference, try_run_inference};

fn fp(x: f64) -> FixedPoint {
    FixedPoint::quantize(x)
}

/// Hand-designed 2→2→1 network used by several tests.
///
/// Layer 1 (2→2): W = [[1, 0], [0, 1]], b = [0, -1]
/// Layer 2 (2→1): W = [[0.5, 1]], b = [0.25]
///
/// For input [2.0, 0.5]:
///   pre-ReLU hidden = [2.0, -0.5] → after ReLU [2.0, 0.0]
///   raw score = 0.5*2 + 1*0 + 0.25 = 1.25
///   Q16.16 raw = 81920
fn network_2_2_1() -> TinyMLP {
    TinyMLP {
        layers: vec![
            DenseLayer {
                weights: vec![fp(1.0), fp(0.0), fp(0.0), fp(1.0)],
                biases: vec![fp(0.0), fp(-1.0)],
                input_size: 2,
                output_size: 2,
            },
            DenseLayer {
                weights: vec![fp(0.5), fp(1.0)],
                biases: vec![fp(0.25)],
                input_size: 2,
                output_size: 1,
            },
        ],
    }
}

#[test]
fn hand_computed_2_2_1_matches_exact_q16_16() {
    let model = Model::TinyMLP(network_2_2_1());
    let out = run_inference(&model, &[fp(2.0), fp(0.5)]);
    // Exact Q16.16 for 1.25: round(1.25 * 2^16) = 81920
    assert_eq!(out.value, 81920);
    assert_eq!(out.scale, 16);
}

#[test]
fn relu_zeroes_negative_hidden_activations() {
    // Without ReLU the second hidden unit stays -0.5 and the final score
    // would be 0.75 (raw 49152). With ReLU it is zeroed → final 1.25.
    let model = Model::TinyMLP(network_2_2_1());
    let out = run_inference(&model, &[fp(2.0), fp(0.5)]);
    assert_eq!(out.value, 81920);
    assert_ne!(out.value, 49152);
}

#[test]
fn shape_mismatch_input_length_returns_error() {
    let model = Model::TinyMLP(network_2_2_1());
    let err = try_run_inference(&model, &[fp(1.0)])
        .expect_err("single feature must not match input_size 2");
    assert_eq!(
        err,
        ZkmlError::FeatureCountMismatch {
            expected: 2,
            got: 1
        }
    );
}

#[test]
fn shape_mismatch_layer_chain_returns_error() {
    let mlp = TinyMLP {
        layers: vec![
            DenseLayer {
                weights: vec![fp(1.0), fp(0.0), fp(0.0), fp(1.0)],
                biases: vec![fp(0.0), fp(0.0)],
                input_size: 2,
                output_size: 2,
            },
            // Deliberately expects 3 inputs while previous layer outputs 2.
            DenseLayer {
                weights: vec![fp(1.0); 3],
                biases: vec![fp(0.0)],
                input_size: 3,
                output_size: 1,
            },
        ],
    };
    assert!(matches!(
        mlp.validate(),
        Err(ZkmlError::InvalidModel(_))
    ));
    let model = Model::TinyMLP(mlp);
    let err = try_run_inference(&model, &[fp(1.0), fp(2.0)])
        .expect_err("unchained layers must be rejected before arithmetic");
    assert!(matches!(err, ZkmlError::InvalidModel(_)));
}
