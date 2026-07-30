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
    assert!(matches!(mlp.validate(), Err(ZkmlError::InvalidModel(_))));
    let model = Model::TinyMLP(mlp);
    let err = try_run_inference(&model, &[fp(1.0), fp(2.0)])
        .expect_err("unchained layers must be rejected before arithmetic");
    assert!(matches!(err, ZkmlError::InvalidModel(_)));
}

/// Maximum allowed |fixed − float| for the golden 2→2→1 network.
///
/// Bound rationale: each Q16.16 multiply/quantize introduces at most
/// ~0.5 ULP ≈ `0.5 / 2^16` absolute error; across two dense layers plus
/// quantization of inputs/weights we pad to `2 / 2^16 ≈ 3.05e-5`, then
/// round up to a comfortable documented ceiling of `1e-4`.
const GOLDEN_ABS_ERR_BOUND: f64 = 1e-4;

fn float_mlp_2_2_1(inputs: &[f64; 2]) -> f64 {
    // Same weights as `network_2_2_1`, evaluated in f64.
    let h0 = 1.0 * inputs[0] + 0.0 * inputs[1] + 0.0;
    let h1 = 0.0 * inputs[0] + 1.0 * inputs[1] + (-1.0);
    let a0 = h0.max(0.0);
    let a1 = h1.max(0.0);
    0.5 * a0 + 1.0 * a1 + 0.25
}

#[test]
fn golden_float_reference_within_documented_bound() {
    let model = Model::TinyMLP(network_2_2_1());
    let inputs_f = [2.0_f64, 0.5];
    let fixed_out = run_inference(&model, &[fp(inputs_f[0]), fp(inputs_f[1])]).dequantize();
    let float_out = float_mlp_2_2_1(&inputs_f);
    let err = (fixed_out - float_out).abs();
    assert!(
        err <= GOLDEN_ABS_ERR_BOUND,
        "fixed={fixed_out} float={float_out} abs_err={err} exceeds bound {GOLDEN_ABS_ERR_BOUND}"
    );
}
