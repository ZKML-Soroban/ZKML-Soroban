//! Commitment helpers binding models and inputs to a proof.
//!
//! The verifier checks that a proof corresponds to a specific model and input
//! by comparing Poseidon commitments. The on-chain side uses the Poseidon host
//! function from CAP-0075; off-chain we expose a stable hashing interface so
//! the prover can compute matching commitments.
//!
//! This implementation uses Poseidon with circomlib-compatible parameters over
//! the BN254 scalar field, matching the Soroban host function configuration.
//!
//! # Hiding
//!
//! [`commit_model`] and [`commit_inputs`] are hiding: each takes an [`Opening`],
//! 32 secret random bytes chosen when the model is registered and kept by
//! whoever proves with it. A plain hash of the parameters would not be: a model
//! with few or predictable parameters, like a small tree with round
//! thresholds, can be recovered from its public commitment by trying candidate
//! models until one matches, and the same goes for input features. With the
//! opening mixed in first, a commitment says nothing about the model to anyone
//! who does not hold the opening, and it still binds the proof to exactly one
//! model.
//!
//! [`commitment_hash`] and [`commit_i64`] stay deterministic. They are generic
//! hashes, used for Merkle trees and identifiers, and must not be used to hide
//! parameters or inputs.

use crate::journal::ModelKind;
use crate::models::{Model, TreeNode};
use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use light_poseidon::{Poseidon, PoseidonHasher};

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// A 32-byte commitment value.
pub use crate::Commitment;

/// The secret that makes [`commit_model`] and [`commit_inputs`] hiding.
///
/// 32 bytes from a cryptographically secure random source, generated once
/// when a model is registered, and kept private by whoever proves with that
/// model: it is needed again for every proof, and anyone who holds it can test
/// guesses against the public commitments. It is read as a little-endian
/// integer reduced modulo the BN254 scalar field, so it keeps more than 250
/// bits of entropy.
pub type Opening = [u8; 32];

/// Domain tag for hiding model commitments. Tags 1 and 2 belonged to the
/// earlier, non-hiding model and input commitments; new tags keep the two
/// schemes from ever producing the same value.
const MODEL_DOMAIN: u64 = 3;

/// Domain tag for hiding input commitments.
const INPUT_DOMAIN: u64 = 4;

/// State size for Poseidon (t=3: rate=2, capacity=1).
/// This matches the circomlib implementation and CAP-0075 defaults.
/// Note: light-poseidon's new_circom(n) creates width t = n + 1, so new_circom(2) creates t=3.
const STATE_SIZE: usize = 3;

/// Compute a Poseidon commitment over a sequence of i64 field elements.
///
/// This uses the circomlib-compatible Poseidon parameters over BN254:
/// - State size t=3 (rate=2, capacity=1)
/// - S-box degree x^5
/// - 8 full rounds, 57 partial rounds
///
/// Domain separation is achieved by prepending the domain tag to the input stream.
/// For inputs exceeding the rate, we use iterative hashing (Merkle-Damgard style):
/// each chunk is hashed with the previous hash as input, chaining the results.
///
/// Deterministic, so not hiding; see [`poseidon_commit_hiding`].
///
/// # Arguments
///
/// * `elements` - Sequence of i64 values to hash
/// * `domain` - Domain tag
fn poseidon_commit(elements: &[i64], domain: u64) -> Commitment {
    poseidon_chain(Fr::from(domain), elements)
}

/// A hiding Poseidon commitment: the chain of [`poseidon_commit`], started from
/// `Poseidon(domain, opening)` instead of the bare domain tag.
///
/// Every later step depends on that first hash, so without the opening the
/// commitment cannot be recomputed for a guessed model or input.
fn poseidon_commit_hiding(elements: &[i64], domain: u64, opening: &Opening) -> Commitment {
    let mut poseidon = Poseidon::<Fr>::new_circom(2).unwrap();
    let blinding = Fr::from_le_bytes_mod_order(opening);
    let start = poseidon.hash(&[Fr::from(domain), blinding]).unwrap();
    poseidon_chain(start, elements)
}

/// Hash `elements` in chunks of the rate, chaining each chunk's hash into the
/// next, starting from `start`.
fn poseidon_chain(start: Fr, elements: &[i64]) -> Commitment {
    let rate = STATE_SIZE - 1; // rate = 2 for t=3

    // Convert i64 elements to Fr field elements with sign preservation
    // For injective mapping: negative values map to (field_order - abs(value))
    // This ensures +w and -w produce different field elements
    let fr_elements: Vec<Fr> = elements
        .iter()
        .map(|&v| {
            if v >= 0 {
                Fr::from(v as u64)
            } else {
                // Map negative to field_order - abs(v) for injective mapping
                let abs_val = v.unsigned_abs();
                Fr::from(0u64) - Fr::from(abs_val)
            }
        })
        .collect();

    // Create Poseidon instance with t=3 (rate=2, capacity=1)
    // new_circom(2) creates width t = 2 + 1 = 3
    let mut poseidon = Poseidon::<Fr>::new_circom(2).unwrap();

    // Iterative hashing: chain hashes for inputs exceeding rate
    let mut current_hash = start;

    for (chunk_index, chunk) in fr_elements.chunks(rate).enumerate() {
        let mut inputs: Vec<Fr> = chunk.to_vec();
        // Pad with zeros if chunk is smaller than rate
        while inputs.len() < rate {
            inputs.push(Fr::from(0u64));
        }
        // Mix previous hash into first element to preserve order
        inputs[0] = inputs[0] + current_hash + Fr::from(chunk_index as u64);
        let chunk_hash = poseidon.hash(&inputs).unwrap();
        current_hash = chunk_hash;
    }

    // Convert Fr to bytes
    let hash_bytes = current_hash.into_bigint().to_bytes_le();
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash_bytes);
    result
}

/// Deterministic hash over little-endian `i64` field elements.
///
/// Not hiding: anyone can recompute it for a guessed sequence. Model and input
/// commitments use [`commit_model`] and [`commit_inputs`], which take an
/// [`Opening`].
///
/// This is implemented using Poseidon with circomlib-compatible parameters
/// over BN254, matching the Soroban host function configuration from CAP-0075.
/// For inputs exceeding the rate, iterative hashing (Merkle-Damgard style) is used.
///
/// # Arguments
///
/// * `elements` - Sequence of i64 values to hash
///
/// # Returns
///
/// 32-byte Poseidon commitment
pub fn commitment_hash(elements: &[i64]) -> Commitment {
    // Use domain 0 for generic commitment_hash (legacy compatibility)
    poseidon_commit(elements, 0)
}

/// Flatten model parameters into the element stream [`commit_model`] hashes.
///
/// Shared by the host prover and the zkVM guest so journal `model_hash`
/// cannot drift from the native commitment.
///
/// The stream is injective: it starts with the model kind and length-prefixes
/// every variable-length part, so two models of different shape cannot flatten
/// to the same sequence. Without this a logistic regression and a decision tree
/// whose fields happened to line up shared a commitment, which is what binds a
/// proof to the model that was registered. Tree nodes also carry a per-node tag
/// (`0` split, `1` leaf) so a leaf value cannot be read as a split's fields.
pub fn model_elements(model: &Model) -> Vec<i64> {
    let mut out = Vec::new();
    // Element 0 is the model kind, so the three variants never share a prefix.
    out.push(ModelKind::of(model) as i64);
    match model {
        Model::LogisticRegression(lr) => {
            out.push(lr.weights.len() as i64);
            out.extend(lr.weights.iter().map(|w| w.value));
            out.push(lr.bias.value);
            out.push(lr.decision_threshold.value);
        }
        Model::DecisionTree(tree) => {
            out.push(tree.num_features as i64);
            out.push(tree.nodes.len() as i64);
            for node in &tree.nodes {
                match node {
                    TreeNode::Split {
                        feature_index,
                        threshold,
                        left,
                        right,
                    } => {
                        out.push(0); // split tag
                        out.push(*feature_index as i64);
                        out.push(threshold.value);
                        out.push(*left as i64);
                        out.push(*right as i64);
                    }
                    TreeNode::Leaf { value } => {
                        out.push(1); // leaf tag
                        out.push(value.value);
                    }
                }
            }
        }
        Model::TinyMLP(mlp) => {
            out.push(mlp.layers.len() as i64);
            for layer in &mlp.layers {
                out.push(layer.weights.len() as i64);
                out.extend(layer.weights.iter().map(|w| w.value));
                out.push(layer.biases.len() as i64);
                out.extend(layer.biases.iter().map(|b| b.value));
                out.push(layer.input_size as i64);
                out.push(layer.output_size as i64);
            }
        }
    }
    out
}

/// Hiding Poseidon commitment to a model: the `model_hash` a verifier is
/// registered with and a proof's journal carries.
///
/// Hashes [`model_elements`] under `opening`, with its own domain tag. The same
/// model under two openings gives unrelated commitments, so the value says
/// nothing about the parameters to anyone without the opening.
pub fn commit_model(model: &Model, opening: &Opening) -> Commitment {
    let elements = model_elements(model);
    poseidon_commit_hiding(&elements, MODEL_DOMAIN, opening)
}

/// Hiding Poseidon commitment to input features: a proof's `input_hash`.
///
/// Uses the model's opening under a different domain tag, so the prover needs
/// one secret per registered model. The same input under the same opening
/// gives the same commitment, which is what lets the verifier's nullifier
/// refuse a second recording of the same inference.
pub fn commit_inputs(features: &[crate::fixed_point::FixedPoint], opening: &Opening) -> Commitment {
    let elements: Vec<i64> = features.iter().map(|f| f.value).collect();
    poseidon_commit_hiding(&elements, INPUT_DOMAIN, opening)
}

/// Fold a sequence of little-endian `i64` field elements into a commitment.
///
/// This is now implemented using Poseidon with circomlib-compatible parameters
/// over BN254, matching the Soroban host function configuration from CAP-0075.
///
/// # Arguments
///
/// * `elements` - Sequence of i64 values to hash
///
/// # Returns
///
/// 32-byte Poseidon commitment
pub fn commit_i64(elements: &[i64]) -> Commitment {
    poseidon_commit(elements, 0)
}

/// Encode a commitment as a 64-character lowercase hex string.
pub fn to_hex(c: &Commitment) -> String {
    let mut s = String::with_capacity(64);
    let hex: &[u8; 16] = b"0123456789abcdef";
    for b in c.iter() {
        s.push(hex[(b >> 4) as usize] as char);
        s.push(hex[(b & 0xf) as usize] as char);
    }
    s
}

/// Decode a 64-character hex string into a commitment, if well-formed.
pub fn from_hex(s: &str) -> Option<Commitment> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests_hex {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let c = commit_i64(&[1, 2, 3, 4]);
        let encoded = to_hex(&c);
        assert_eq!(encoded.len(), 64);
        assert_eq!(from_hex(&encoded), Some(c));
    }

    #[test]
    fn from_hex_rejects_bad_length() {
        assert_eq!(from_hex("abcd"), None);
    }
}

#[cfg(test)]
mod tests_stability {
    use super::*;

    #[test]
    fn commitment_is_stable() {
        assert_eq!(commit_i64(&[1, 2, 3]), commit_i64(&[1, 2, 3]));
    }

    #[test]
    fn commitment_is_order_sensitive() {
        assert_ne!(commit_i64(&[1, 2, 3]), commit_i64(&[3, 2, 1]));
    }

    #[test]
    fn empty_commitment_is_deterministic() {
        let empty1 = commit_i64(&[]);
        let empty2 = commit_i64(&[]);
        assert_eq!(empty1, empty2);
    }

    #[test]
    fn domain_separation_produces_different_hashes() {
        let elements = vec![1, 2, 3];
        let hash_domain1 = poseidon_commit(&elements, 1);
        let hash_domain2 = poseidon_commit(&elements, 2);
        assert_ne!(hash_domain1, hash_domain2);
    }
}

#[cfg(test)]
mod tests_hiding {
    use super::*;
    use crate::fixed_point::FixedPoint;
    use crate::models::{LogisticRegression, Model};

    const OPENING: Opening = [7u8; 32];

    fn model(weight: i64) -> Model {
        Model::LogisticRegression(LogisticRegression {
            weights: vec![
                FixedPoint::from_raw(weight, 16),
                FixedPoint::from_raw(2, 16),
            ],
            bias: FixedPoint::from_raw(3, 16),
            decision_threshold: FixedPoint::from_raw(0, 16),
        })
    }

    #[test]
    fn the_same_model_and_opening_give_the_same_commitment() {
        assert_eq!(
            commit_model(&model(1), &OPENING),
            commit_model(&model(1), &OPENING)
        );
    }

    #[test]
    fn another_opening_gives_an_unrelated_commitment() {
        let mut other = OPENING;
        other[0] ^= 1;
        assert_ne!(
            commit_model(&model(1), &OPENING),
            commit_model(&model(1), &other)
        );
        let inputs = [FixedPoint::from_raw(5, 16)];
        assert_ne!(
            commit_inputs(&inputs, &OPENING),
            commit_inputs(&inputs, &other)
        );
    }

    /// The vulnerability this scheme closes: with only public data, an
    /// attacker who enumerates candidate models finds the one whose plain hash
    /// matches. Without the opening, no candidate's plain hash matches.
    #[test]
    fn a_plain_hash_of_the_right_model_does_not_match() {
        let committed = commit_model(&model(1), &OPENING);
        for weight in -100..=100 {
            let elements = model_elements(&model(weight));
            assert_ne!(commitment_hash(&elements), committed, "weight {weight}");
            assert_ne!(
                poseidon_commit(&elements, MODEL_DOMAIN),
                committed,
                "weight {weight}"
            );
        }
    }

    #[test]
    fn model_and_input_commitments_use_separate_domains() {
        let elements = [1i64, 2, 3];
        assert_ne!(
            poseidon_commit_hiding(&elements, MODEL_DOMAIN, &OPENING),
            poseidon_commit_hiding(&elements, INPUT_DOMAIN, &OPENING)
        );
    }

    #[test]
    fn a_hiding_commitment_differs_from_the_plain_chain() {
        let elements = [1i64, 2, 3];
        assert_ne!(
            poseidon_commit_hiding(&elements, MODEL_DOMAIN, &OPENING),
            poseidon_commit(&elements, MODEL_DOMAIN)
        );
    }

    #[test]
    fn the_plain_chain_is_unchanged() {
        // The refactor that added the hiding start must not move the
        // deterministic hashes the Merkle tree and identifiers rely on.
        assert_eq!(commit_i64(&[1, 2, 3]), poseidon_commit(&[1, 2, 3], 0));
        assert_eq!(
            poseidon_chain(Fr::from(0u64), &[1, 2, 3]),
            commit_i64(&[1, 2, 3])
        );
    }
}

#[cfg(test)]
mod tests_model_elements {
    use super::*;
    use crate::fixed_point::FixedPoint;
    use crate::models::{DecisionTree, LogisticRegression, Model, TreeNode};

    #[test]
    fn logistic_flattens_weights_then_bias() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::from_raw(1, 16), FixedPoint::from_raw(2, 16)],
            bias: FixedPoint::from_raw(3, 16),
            decision_threshold: FixedPoint::from_raw(0, 16),
        });
        // kind=1 (LR), weights.len()=2, w0=1, w1=2, bias=3, threshold=0
        assert_eq!(model_elements(&model), vec![1, 2, 1, 2, 3, 0]);
    }

    #[test]
    fn tree_flattens_features_and_nodes() {
        let model = Model::DecisionTree(DecisionTree {
            num_features: 1,
            nodes: vec![
                TreeNode::Split {
                    feature_index: 0,
                    threshold: FixedPoint::from_raw(10, 16),
                    left: 1,
                    right: 2,
                },
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(0, 16),
                },
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(1, 16),
                },
            ],
        });
        // kind=0 (tree), num_features=1, nodes.len()=3,
        // split(tag 0): feature 0, threshold 10, left 1, right 2,
        // leaf(tag 1): 0, leaf(tag 1): 1
        assert_eq!(
            model_elements(&model),
            vec![0, 1, 3, 0, 0, 10, 1, 2, 1, 0, 1, 1]
        );
    }

    #[test]
    fn commit_model_produces_deterministic_hash() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::from_raw(1, 16), FixedPoint::from_raw(2, 16)],
            bias: FixedPoint::from_raw(3, 16),
            decision_threshold: FixedPoint::from_raw(0, 16),
        });
        let hash1 = commit_model(&model, &[1; 32]);
        let hash2 = commit_model(&model, &[1; 32]);
        assert_eq!(hash1, hash2);
    }

    /// Two models of different shape must not share a commitment.
    ///
    /// A logistic regression with weights `[10, 20]`, bias `30`, threshold `40`
    /// and a decision tree with `num_features = 10` and three leaves `20, 30,
    /// 40` used to flatten to the same `[10, 20, 30, 40]` and hash to the same
    /// commitment. The model kind and the length prefixes keep them apart.
    #[test]
    fn different_shapes_do_not_share_a_commitment() {
        let lr = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::from_raw(10, 16), FixedPoint::from_raw(20, 16)],
            bias: FixedPoint::from_raw(30, 16),
            decision_threshold: FixedPoint::from_raw(40, 16),
        });
        let tree = Model::DecisionTree(DecisionTree {
            num_features: 10,
            nodes: vec![
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(20, 16),
                },
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(30, 16),
                },
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(40, 16),
                },
            ],
        });

        assert_ne!(
            model_elements(&lr),
            model_elements(&tree),
            "different model shapes must not flatten to the same stream"
        );
        assert_ne!(
            commit_model(&lr, &[1; 32]),
            commit_model(&tree, &[1; 32]),
            "different model shapes must not share a commitment"
        );
    }

    #[test]
    fn commit_inputs_produces_deterministic_hash() {
        let features = vec![
            FixedPoint::from_raw(1, 16),
            FixedPoint::from_raw(2, 16),
            FixedPoint::from_raw(3, 16),
        ];
        let hash1 = commit_inputs(&features, &[1; 32]);
        let hash2 = commit_inputs(&features, &[1; 32]);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn changing_model_parameter_changes_commitment() {
        let model1 = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::from_raw(1, 16), FixedPoint::from_raw(2, 16)],
            bias: FixedPoint::from_raw(3, 16),
            decision_threshold: FixedPoint::from_raw(0, 16),
        });
        let model2 = Model::LogisticRegression(LogisticRegression {
            weights: vec![FixedPoint::from_raw(1, 16), FixedPoint::from_raw(99, 16)], // Changed weight
            bias: FixedPoint::from_raw(3, 16),
            decision_threshold: FixedPoint::from_raw(0, 16),
        });
        assert_ne!(
            commit_model(&model1, &[1; 32]),
            commit_model(&model2, &[1; 32])
        );
    }

    #[test]
    fn changing_input_changes_commitment() {
        let features1 = vec![FixedPoint::from_raw(1, 16), FixedPoint::from_raw(2, 16)];
        let features2 = vec![FixedPoint::from_raw(1, 16), FixedPoint::from_raw(99, 16)]; // Changed input
        assert_ne!(
            commit_inputs(&features1, &[1; 32]),
            commit_inputs(&features2, &[1; 32])
        );
    }
}

#[cfg(test)]
mod tests_snapshot {
    use super::*;
    use crate::fixed_point::FixedPoint;
    use crate::models::{DecisionTree, DenseLayer, LogisticRegression, Model, TinyMLP, TreeNode};
    use insta::assert_debug_snapshot;

    /// A fixed, public opening: these snapshots pin the scheme, not a secret.
    const SNAPSHOT_OPENING: Opening = [0x5a; 32];

    #[test]
    fn snapshot_logistic_regression_commitment() {
        let model = Model::LogisticRegression(LogisticRegression {
            weights: vec![
                FixedPoint::from_raw(100, 16),
                FixedPoint::from_raw(200, 16),
                FixedPoint::from_raw(300, 16),
            ],
            bias: FixedPoint::from_raw(50, 16),
            decision_threshold: FixedPoint::from_raw(0, 16),
        });
        let hash = commit_model(&model, &SNAPSHOT_OPENING);
        assert_debug_snapshot!(hash);
    }

    #[test]
    fn snapshot_decision_tree_commitment() {
        let model = Model::DecisionTree(DecisionTree {
            num_features: 2,
            nodes: vec![
                TreeNode::Split {
                    feature_index: 0,
                    threshold: FixedPoint::from_raw(150, 16),
                    left: 1,
                    right: 2,
                },
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(0, 16),
                },
                TreeNode::Split {
                    feature_index: 1,
                    threshold: FixedPoint::from_raw(200, 16),
                    left: 3,
                    right: 4,
                },
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(1, 16),
                },
                TreeNode::Leaf {
                    value: FixedPoint::from_raw(2, 16),
                },
            ],
        });
        let hash = commit_model(&model, &SNAPSHOT_OPENING);
        assert_debug_snapshot!(hash);
    }

    #[test]
    fn snapshot_tiny_mlp_commitment() {
        let model = Model::TinyMLP(TinyMLP {
            layers: vec![DenseLayer {
                weights: vec![FixedPoint::from_raw(10, 16), FixedPoint::from_raw(20, 16)],
                biases: vec![FixedPoint::from_raw(5, 16)],
                input_size: 2,
                output_size: 1,
            }],
        });
        let hash = commit_model(&model, &SNAPSHOT_OPENING);
        assert_debug_snapshot!(hash);
    }

    #[test]
    fn snapshot_input_commitment() {
        let features = vec![
            FixedPoint::from_raw(100, 16),
            FixedPoint::from_raw(200, 16),
            FixedPoint::from_raw(300, 16),
        ];
        let hash = commit_inputs(&features, &SNAPSHOT_OPENING);
        assert_debug_snapshot!(hash);
    }

    #[test]
    fn snapshot_empty_input_commitment() {
        let features: Vec<FixedPoint> = vec![];
        let hash = commit_inputs(&features, &SNAPSHOT_OPENING);
        assert_debug_snapshot!(hash);
    }
}

#[cfg(test)]
mod test_determinism {
    use super::*;

    /// Determinism test for Poseidon commitment
    ///
    /// This test verifies that the off-chain implementation produces consistent
    /// digests for the same input. This is a basic sanity check, not a cross-check
    /// against the on-chain CAP-0075 host function.
    ///
    /// TODO: To implement a true on-chain equivalence test, we need:
    /// 1. Access to Soroban testnet/devnet to run the CAP-0075 host function
    /// 2. OR integration with soroban-poseidon crate to compute reference digest
    /// 3. A known test vector: fixed input -> expected digest from on-chain source
    #[test]
    fn poseidon_commitment_is_deterministic() {
        let input = [1i64, 2i64, 3i64];
        let digest1 = poseidon_commit(&input, 1);
        let digest2 = poseidon_commit(&input, 1);
        assert_eq!(digest1, digest2, "commitment should be deterministic");
    }
}
