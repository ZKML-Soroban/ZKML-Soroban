//! A small binary Merkle tree over commitments.
//!
//! Used to commit to a model whose parameters are chunked into leaves, so an
//! individual chunk can later be opened without revealing the whole model.

use crate::commitment::{commit_i64, Commitment};

/// Domain tag for leaf hashing.
const LEAF_DOMAIN: u64 = 10;

/// Domain tag for internal node hashing.
const INTERNAL_DOMAIN: u64 = 11;

/// A Merkle proof for a leaf at a specific index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProof {
    /// The sibling hashes at each level of the tree, from bottom to top.
    pub siblings: Vec<Commitment>,
    /// The index of the leaf in the original tree.
    pub index: usize,
}

/// Compute the Merkle root of a list of leaf commitments.
///
/// Odd levels duplicate the last node (standard padding). An empty input
/// returns the all-zero commitment.
///
/// Uses domain separation: leaves are hashed with LEAF_DOMAIN, internal nodes
/// with INTERNAL_DOMAIN to prevent second-preimage attacks.
pub fn merkle_root(leaves: &[Commitment]) -> Commitment {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    // Hash leaves with domain separation
    let mut level: Vec<Commitment> = leaves.iter().map(|leaf| hash_leaf(leaf)).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let a = pair[0];
            let b = if pair.len() == 2 { pair[1] } else { pair[0] };
            next.push(hash_internal(&a, &b));
        }
        level = next;
    }
    level[0]
}

fn hash_leaf(leaf: &Commitment) -> Commitment {
    let mut elements = [0i64; 5];
    // Add domain tag
    elements[0] = LEAF_DOMAIN as i64;
    // Add leaf bytes
    for i in 0..4 {
        elements[i + 1] = i64::from_le_bytes(leaf[i * 8..i * 8 + 8].try_into().unwrap());
    }
    commit_i64(&elements)
}

fn hash_internal(a: &Commitment, b: &Commitment) -> Commitment {
    let mut elements = [0i64; 9];
    // Add domain tag
    elements[0] = INTERNAL_DOMAIN as i64;
    // Add both nodes
    for i in 0..4 {
        elements[i + 1] = i64::from_le_bytes(a[i * 8..i * 8 + 8].try_into().unwrap());
        elements[i + 5] = i64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
    }
    commit_i64(&elements)
}

/// Generate a Merkle proof for the leaf at the given index.
///
/// Returns None if the index is out of bounds or the tree is empty.
pub fn generate_proof(leaves: &[Commitment], index: usize) -> Option<MerkleProof> {
    if leaves.is_empty() || index >= leaves.len() {
        return None;
    }

    let mut siblings = Vec::new();
    let mut level: Vec<Commitment> = leaves.iter().map(|leaf| hash_leaf(leaf)).collect();
    let mut current_index = index;

    while level.len() > 1 {
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };

        // Get sibling if it exists, otherwise duplicate the current node (padding)
        let sibling = if sibling_index < level.len() {
            level[sibling_index]
        } else {
            level[current_index]
        };

        siblings.push(sibling);

        // Move up the tree
        current_index = current_index / 2;
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let a = pair[0];
            let b = if pair.len() == 2 { pair[1] } else { pair[0] };
            next.push(hash_internal(&a, &b));
        }
        level = next;
    }

    Some(MerkleProof { siblings, index })
}

/// Verify a Merkle proof.
///
/// Returns true if the leaf is included at the given index under the root.
pub fn verify_proof(root: &Commitment, leaf: &Commitment, proof: &MerkleProof) -> bool {
    let mut computed = hash_leaf(leaf);
    let mut current_index = proof.index;

    for sibling in &proof.siblings {
        let (left, right) = if current_index % 2 == 0 {
            (computed, *sibling)
        } else {
            (*sibling, computed)
        };
        computed = hash_internal(&left, &right);
        current_index = current_index / 2;
    }

    computed == *root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_deterministic() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        assert_eq!(merkle_root(&leaves), merkle_root(&leaves));
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(merkle_root(&[]), [0u8; 32]);
    }

    #[test]
    fn order_matters() {
        let a = vec![[1u8; 32], [2u8; 32]];
        let b = vec![[2u8; 32], [1u8; 32]];
        assert_ne!(merkle_root(&a), merkle_root(&b));
    }

    #[test]
    fn proof_verifies_valid_inclusion() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let root = merkle_root(&leaves);
        let proof = generate_proof(&leaves, 2).unwrap();
        assert!(verify_proof(&root, &leaves[2], &proof));
    }

    #[test]
    fn proof_fails_for_tampered_leaf() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let root = merkle_root(&leaves);
        let proof = generate_proof(&leaves, 2).unwrap();
        let tampered_leaf = [99u8; 32];
        assert!(!verify_proof(&root, &tampered_leaf, &proof));
    }

    #[test]
    fn proof_fails_for_wrong_index() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let root = merkle_root(&leaves);
        let proof = generate_proof(&leaves, 2).unwrap();
        let mut wrong_proof = proof.clone();
        wrong_proof.index = 1;
        assert!(!verify_proof(&root, &leaves[2], &wrong_proof));
    }

    #[test]
    fn proof_fails_for_tampered_sibling() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let root = merkle_root(&leaves);
        let mut proof = generate_proof(&leaves, 2).unwrap();
        proof.siblings[0] = [99u8; 32];
        assert!(!verify_proof(&root, &leaves[2], &proof));
    }

    #[test]
    fn proof_generation_fails_for_empty_tree() {
        let leaves: Vec<Commitment> = vec![];
        assert!(generate_proof(&leaves, 0).is_none());
    }

    #[test]
    fn proof_generation_fails_for_out_of_bounds_index() {
        let leaves = vec![[1u8; 32], [2u8; 32]];
        assert!(generate_proof(&leaves, 5).is_none());
    }

    #[test]
    fn proof_works_for_single_leaf() {
        let leaves = vec![[1u8; 32]];
        let root = merkle_root(&leaves);
        let proof = generate_proof(&leaves, 0).unwrap();
        assert!(verify_proof(&root, &leaves[0], &proof));
    }

    #[test]
    fn proof_works_for_odd_number_of_leaves() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let root = merkle_root(&leaves);
        let proof = generate_proof(&leaves, 1).unwrap();
        assert!(verify_proof(&root, &leaves[1], &proof));
    }

    #[test]
    fn domain_separation_prevents_second_preimage() {
        // Without domain separation, a leaf value could be reinterpreted as an internal node
        // With domain separation, this should be impossible
        let leaf = [1u8; 32];
        let leaf_hash = hash_leaf(&leaf);

        // Try to create a tree where the leaf hash appears as an internal node
        let leaves = vec![leaf, [2u8; 32]];
        let root = merkle_root(&leaves);

        // The leaf hash should not equal any internal node hash
        // This is a basic sanity check - a full second-preimage test would require
        // finding a collision, which is computationally infeasible
        assert_ne!(leaf_hash, root);
    }

    #[test]
    fn leaf_and_internal_hashes_differ() {
        let leaf = [1u8; 32];
        let leaf_hash = hash_leaf(&leaf);
        let internal_hash = hash_internal(&leaf, &leaf);
        assert_ne!(leaf_hash, internal_hash);
    }

    #[test]
    fn domain_tags_produce_different_hashes() {
        let leaf = [1u8; 32];
        let leaf_hash = hash_leaf(&leaf);

        // Manually construct what a non-domain-separated hash would look like
        let mut elements = [0i64; 4];
        for i in 0..4 {
            elements[i] = i64::from_le_bytes(leaf[i * 8..i * 8 + 8].try_into().unwrap());
        }
        let no_domain_hash = commit_i64(&elements);

        // Domain-separated hash should be different
        assert_ne!(leaf_hash, no_domain_hash);
    }

    #[test]
    fn proof_index_preserved() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let proof = generate_proof(&leaves, 2).unwrap();
        assert_eq!(proof.index, 2);
    }

    #[test]
    fn proof_siblings_length_matches_tree_depth() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let proof = generate_proof(&leaves, 0).unwrap();
        // Tree with 4 leaves has depth 2 (4 -> 2 -> 1)
        assert_eq!(proof.siblings.len(), 2);
    }
}

#[cfg(test)]
mod tests_snapshot {
    use super::*;
    use insta::assert_debug_snapshot;

    #[test]
    fn snapshot_merkle_root() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let root = merkle_root(&leaves);
        assert_debug_snapshot!(root);
    }

    #[test]
    fn snapshot_merkle_root_single_leaf() {
        let leaves = vec![[1u8; 32]];
        let root = merkle_root(&leaves);
        assert_debug_snapshot!(root);
    }

    #[test]
    fn snapshot_merkle_root_odd_leaves() {
        let leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let root = merkle_root(&leaves);
        assert_debug_snapshot!(root);
    }
}
