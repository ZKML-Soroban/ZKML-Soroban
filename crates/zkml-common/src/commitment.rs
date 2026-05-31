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
