//! A small binary Merkle tree over commitments.
//!
//! Used to commit to a model whose parameters are chunked into leaves, so an
//! individual chunk can later be opened without revealing the whole model.

use crate::commitment::{commit_i64, Commitment};

/// Compute the Merkle root of a list of leaf commitments.
///
/// Odd levels duplicate the last node (standard padding). An empty input
/// returns the all-zero commitment.
pub fn merkle_root(leaves: &[Commitment]) -> Commitment {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    let mut level: Vec<Commitment> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let a = pair[0];
            let b = if pair.len() == 2 { pair[1] } else { pair[0] };
            next.push(hash_pair(&a, &b));
        }
        level = next;
    }
    level[0]
}

fn hash_pair(a: &Commitment, b: &Commitment) -> Commitment {
    let mut elements = [0i64; 8];
    for i in 0..4 {
        elements[i] = i64::from_le_bytes(a[i * 8..i * 8 + 8].try_into().unwrap());
        elements[i + 4] = i64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
    }
    commit_i64(&elements)
}
