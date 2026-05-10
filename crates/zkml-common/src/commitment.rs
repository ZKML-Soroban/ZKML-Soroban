//! Commitment helpers binding models and inputs to a proof.
//!
//! The verifier checks that a proof corresponds to a specific model and input
//! by comparing Poseidon commitments. The on-chain side uses the Poseidon host
//! function from CAP-0075; off-chain we expose a stable hashing interface so
//! the prover can compute matching commitments.

/// A 32-byte commitment value.
pub type Commitment = [u8; 32];

/// Fold a sequence of little-endian `i64` field elements into a commitment.
///
/// This is a placeholder construction (a simple mixing function) that will be
/// replaced by a Poseidon sponge matching the on-chain host function. It is
/// deterministic so the prover and tests agree.
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
