//! Property-based differential test: native inference vs zkVM guest journal.
//!
//! Runs in the `zkvm` CI job (`RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm`).
//! Case count is small because each case builds a guest receipt.

#![cfg(feature = "zkvm")]

use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::try_run_inference;
use zkml_common::models::{DecisionTree, DenseLayer, LogisticRegression, Model, TinyMLP, TreeNode};
use zkml_prover::prover::generate_receipt;

fn small_fp() -> impl Strategy<Value = FixedPoint> {
    (-1.0f64..1.0f64).prop_map(FixedPoint::quantize)
}

fn logistic_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    (1usize..=2).prop_flat_map(|n| {
        (
            prop::collection::vec(small_fp(), n),
            small_fp(),
            prop::collection::vec(small_fp(), n),
        )
            .prop_map(|(weights, bias, inputs)| {
                (
                    Model::LogisticRegression(LogisticRegression { weights, bias }),
                    inputs,
                )
            })
    })
}

fn tree_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    (small_fp(), small_fp(), small_fp(), small_fp()).prop_map(
        |(threshold, left_v, right_v, input)| {
            let tree = DecisionTree {
                num_features: 1,
                nodes: vec![
                    TreeNode::Split {
                        feature_index: 0,
                        threshold,
                        left: 1,
                        right: 2,
                    },
                    TreeNode::Leaf { value: left_v },
                    TreeNode::Leaf { value: right_v },
                ],
            };
            (Model::DecisionTree(tree), vec![input])
        },
    )
}

fn mlp_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    // 2→1→1 keeps guest cycles small. Both inputs and the output bias are
    // drawn from `small_fp` so this is not a 1-D slice of MLP space.
    (
        prop::collection::vec(small_fp(), 2),
        small_fp(),
        small_fp(),
        small_fp(),
        small_fp(),
        small_fp(),
    )
        .prop_map(|(w0, b0, w1, b1, x0, x1)| {
            let hidden = DenseLayer {
                weights: w0,
                biases: vec![b0],
                input_size: 2,
                output_size: 1,
            };
            let out = DenseLayer {
                weights: vec![w1],
                biases: vec![b1],
                input_size: 1,
                output_size: 1,
            };
            (
                Model::TinyMLP(TinyMLP {
                    layers: vec![hidden, out],
                }),
                vec![x0, x1],
            )
        })
}

fn any_valid_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    prop_oneof![logistic_case(), tree_case(), mlp_case()]
}

fn ensure_risc0_dev_mode() {
    use std::sync::Once;
    static SET: Once = Once::new();
    SET.call_once(|| {
        // CI already exports RISC0_DEV_MODE=1. Set it once so local
        // `cargo test -p zkml-prover --features zkvm` still uses fake receipts.
        std::env::set_var("RISC0_DEV_MODE", "1");
    });
}

fn zkvm_proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: 8,
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("tests"))),
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(zkvm_proptest_config())]

    /// Guest journal output matches native `try_run_inference` on random models.
    #[test]
    fn guest_journal_matches_native(case in any_valid_case()) {
        ensure_risc0_dev_mode();
        let (model, inputs) = case;
        let native = try_run_inference(&model, &inputs);
        prop_assert!(native.is_ok(), "try_run_inference returned {native:?}");
        let native_out = native.unwrap();
        let receipt = generate_receipt(&model, &inputs);
        prop_assert!(
            receipt.is_ok(),
            "generate_receipt failed: {:?}",
            receipt.as_ref().err()
        );
        let (_receipt, journal) = receipt.unwrap();
        prop_assert_eq!(journal.output, native_out.value);
    }
}
