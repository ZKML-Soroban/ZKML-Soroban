//! Property-based tests for the inference engine.
//!
//! Complements `fixed_point_props.rs`. These cases stay native (no RISC Zero)
//! so they run in the workspace `test` CI job.

use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use zkml_common::activation::relu;
use zkml_common::fixed_point::FixedPoint;
use zkml_common::inference::{argmax, try_run_inference};
use zkml_common::models::{DecisionTree, DenseLayer, LogisticRegression, Model, TinyMLP, TreeNode};

/// Weights/inputs small enough that Q16.16 products stay well inside i64.
fn small_fp() -> impl Strategy<Value = FixedPoint> {
    (-2.0f64..2.0f64).prop_map(FixedPoint::quantize)
}

fn logistic_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    (1usize..=4).prop_flat_map(|n| {
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

/// Depth-1 stump: always acyclic, every path reaches a leaf, no orphans.
fn tree_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    (1usize..=3).prop_flat_map(|n| {
        (
            0..n,
            small_fp(),
            small_fp(),
            small_fp(),
            prop::collection::vec(small_fp(), n),
        )
            .prop_map(move |(feat, threshold, left_v, right_v, inputs)| {
                let tree = DecisionTree {
                    num_features: n,
                    nodes: vec![
                        TreeNode::Split {
                            feature_index: feat,
                            threshold,
                            left: 1,
                            right: 2,
                        },
                        TreeNode::Leaf { value: left_v },
                        TreeNode::Leaf { value: right_v },
                    ],
                };
                (Model::DecisionTree(tree), inputs)
            })
    })
}

fn dense(in_s: usize, out_s: usize) -> impl Strategy<Value = DenseLayer> {
    (
        prop::collection::vec(small_fp(), in_s * out_s),
        prop::collection::vec(small_fp(), out_s),
    )
        .prop_map(move |(weights, biases)| DenseLayer {
            weights,
            biases,
            input_size: in_s,
            output_size: out_s,
        })
}

fn mlp_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    (1usize..=3, 1usize..=3).prop_flat_map(|(in_s, hidden)| {
        (
            dense(in_s, hidden),
            dense(hidden, 1),
            prop::collection::vec(small_fp(), in_s),
        )
            .prop_map(|(h, out, inputs)| {
                (
                    Model::TinyMLP(TinyMLP {
                        layers: vec![h, out],
                    }),
                    inputs,
                )
            })
    })
}

fn any_valid_case() -> impl Strategy<Value = (Model, Vec<FixedPoint>)> {
    prop_oneof![logistic_case(), tree_case(), mlp_case()]
}

fn inference_proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("tests"))),
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(inference_proptest_config())]

    /// `try_run_inference` is deterministic and never panics on valid models.
    #[test]
    fn try_run_inference_is_deterministic_and_infallible(case in any_valid_case()) {
        let (model, inputs) = case;
        if let Model::DecisionTree(t) = &model {
            prop_assume!(t.validate().is_ok());
        }
        if let Model::TinyMLP(m) = &model {
            prop_assume!(m.validate().is_ok());
        }
        let a = try_run_inference(&model, &inputs);
        let b = try_run_inference(&model, &inputs);
        prop_assert!(a.is_ok(), "try_run_inference returned {a:?}");
        prop_assert_eq!(a, b);
    }

    /// Feature-count mismatch is an error, not a panic.
    #[test]
    fn feature_mismatch_is_err_not_panic(case in any_valid_case()) {
        let (model, mut inputs) = case;
        prop_assume!(!inputs.is_empty());
        inputs.push(FixedPoint::quantize(0.0));
        let result = std::panic::catch_unwind(|| try_run_inference(&model, &inputs));
        prop_assert!(result.is_ok(), "try_run_inference panicked");
        prop_assert!(result.unwrap().is_err());
    }

    /// ReLU is monotone and zeros negatives.
    #[test]
    fn relu_is_monotone((x, y) in (small_fp(), small_fp())) {
        let rx = relu(x);
        let ry = relu(y);
        if x.value <= y.value {
            prop_assert!(rx.value <= ry.value);
        }
        prop_assert!(rx.value >= 0);
        if x.value < 0 {
            prop_assert_eq!(rx.value, 0);
        } else {
            prop_assert_eq!(rx.value, x.value);
        }
    }

    /// Argmax is a stable function of the logit vector (including ties).
    #[test]
    fn argmax_is_stable(values in prop::collection::vec(small_fp(), 1..=6)) {
        let a = argmax(&values);
        let b = argmax(&values);
        prop_assert_eq!(a, b);
        if let Some(i) = a {
            let max_v = values[i].value;
            prop_assert!(values.iter().all(|v| v.value <= max_v));
        }
    }
}
