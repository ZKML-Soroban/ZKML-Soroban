//! Commitment helpers binding models and inputs to a proof.
//!
//! The verifier checks that a proof corresponds to a specific model and input
//! by comparing Poseidon commitments. The on-chain side uses the Poseidon host
//! function from CAP-0075; off-chain we expose a stable hashing interface so
//! the prover can compute matching commitments.

use crate::models::{Model, TreeNode};

/// A 32-byte commitment value.
pub type Commitment = [u8; 32];

/// Canonical commitment over little-endian `i64` field elements.
///
/// Host and zkVM guest must call this same function so journal public inputs
/// match native cross-checks.
///
/// # Security
///
/// **This function is not collision-resistant.** The current body is a
/// wrapping add/multiply mixer (`commit_i64`) with no pretence of
/// cryptographic hardness. Values produced here must **not** gate any
/// on-chain trust or access decision (authorization, model binding for
/// production verification, etc.) until issue #13 replaces this with a
/// Poseidon sponge matching CAP-0075. Using these digests in a STARK
/// journal is fine for Phase 1 plumbing and native cross-checks only.
///
/// # TODO (issue #13)
///
/// Replace the body with a Poseidon sponge matching CAP-0075 on-chain
/// host functions. The current implementation is a deterministic stand-in
/// so prover and tests agree until Poseidon lands.
pub fn commitment_hash(elements: &[i64]) -> Commitment {
    // TODO(#13): Poseidon commitments for model and input binding (CAP-0075).
    commit_i64(elements)
}

/// Flatten model parameters into the element stream used by
/// [`commitment_hash`] for model binding.
///
/// Shared by the host prover and the zkVM guest so journal `model_hash`
/// cannot drift from native `model_commitment`.
pub fn model_elements(model: &Model) -> Vec<i64> {
    let mut out = Vec::new();
    match model {
        Model::LogisticRegression(lr) => {
            out.extend(lr.weights.iter().map(|w| w.value));
            out.push(lr.bias.value);
        }
        Model::DecisionTree(tree) => {
            out.push(tree.num_features as i64);
            for node in &tree.nodes {
                match node {
                    TreeNode::Split {
                        feature_index,
                        threshold,
                        left,
                        right,
                    } => {
                        out.push(*feature_index as i64);
                        out.push(threshold.value);
                        out.push(*left as i64);
                        out.push(*right as i64);
                    }
                    TreeNode::Leaf { value } => out.push(value.value),
                }
            }
        }
        Model::TinyMLP(mlp) => {
            for layer in &mlp.layers {
                out.extend(layer.weights.iter().map(|w| w.value));
                out.extend(layer.biases.iter().map(|b| b.value));
                out.push(layer.input_size as i64);
                out.push(layer.output_size as i64);
            }
        }
    }
    out
}

/// Fold a sequence of little-endian `i64` field elements into a commitment.
///
/// This is a placeholder construction (a simple mixing function) that will be
/// replaced by a Poseidon sponge matching the on-chain host function. Prefer
/// [`commitment_hash`] at call sites that must stay aligned with the guest.
///
/// # Security
///
/// Same caveats as [`commitment_hash`]: not collision-resistant; do not use
/// for production on-chain trust decisions until issue #13.
pub fn commit_i64(elements: &[i64]) -> Commitment {
    let mut state: [u8; 32] = [0u8; 32];
    for (k, e) in elements.iter().enumerate() {
        let bytes = e.to_le_bytes();
        for (i, b) in bytes.iter().enumerate() {
            let idx = (k + i) % 32;
            state[idx] = state[idx].wrapping_add(*b).wrapping_mul(31).wrapping_add(7);
        }
    }
    state
}

/// Encode a commitment as a 64-character lowercase hex string.
pub fn to_hex(c: &Commitment) -> String {
    let mut s = String::with_capacity(64);
    for b in c.iter() {
        s.push_str(&format!("{b:02x}"));
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
    fn empty_commitment_is_zero() {
        assert_eq!(commit_i64(&[]), [0u8; 32]);
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
        });
        assert_eq!(model_elements(&model), vec![1, 2, 3]);
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
        assert_eq!(model_elements(&model), vec![1, 0, 10, 1, 2, 0, 1]);
    }
}
