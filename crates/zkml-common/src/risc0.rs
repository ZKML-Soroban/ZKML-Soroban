//! RISC Zero digest arithmetic, reimplemented without the RISC Zero crates.
//!
//! A Groth16 receipt produced by RISC Zero does not carry the journal as a
//! public input. It carries five field elements:
//!
//! ```text
//! [control_root_lo, control_root_hi, claim_digest_lo, claim_digest_hi, bn254_control_id]
//! ```
//!
//! where `claim_digest` binds the image id (which program ran) to the journal
//! (what it output). A verifier therefore has to recompute the claim digest
//! from the journal it was given. This module provides exactly that arithmetic
//! so both the prover and a `no_std` verifier can do it without depending on
//! `risc0-zkvm`.
//!
//! Every function here mirrors the reference implementation in `risc0-binfmt`
//! and `risc0-zkvm` 3.0.x; `crates/zkml-prover/tests/risc0_digests.rs`
//! cross-checks the values against those crates.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// A 32-byte SHA-256 digest, in RISC Zero byte order.
pub type Digest = [u8; 32];

/// The all-zero digest, used for pruned or absent fields.
pub const ZERO_DIGEST: Digest = [0u8; 32];

/// Length of an encoded Groth16 seal: 4-byte selector plus 256-byte proof.
pub const SEAL_LEN: usize = 4 + 256;

/// A SHA-256 implementation, supplied by the caller.
///
/// The digests below are built from many small hashes, and where that runs
/// decides which implementation is right. Off-chain, `sha2` compiled into the
/// binary is fine. Inside a Soroban contract, `env.crypto().sha256()` is a host
/// function and costs a fraction of the same work done in WASM. Injecting the
/// hash lets one implementation of the digest arithmetic serve both, which
/// matters because the two must agree exactly or nothing verifies.
pub trait Sha256: Copy {
    /// Hash `bytes` and return the 32-byte digest.
    fn hash(&self, bytes: &[u8]) -> Digest;
}

impl<F: Fn(&[u8]) -> Digest + Copy> Sha256 for F {
    fn hash(&self, bytes: &[u8]) -> Digest {
        self(bytes)
    }
}

/// The `sha2` implementation, for callers that are not inside a contract.
#[cfg(feature = "sha2")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sha2Hasher;

#[cfg(feature = "sha2")]
impl Sha256 for Sha2Hasher {
    fn hash(&self, bytes: &[u8]) -> Digest {
        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::new();
        hasher.update(bytes);
        hasher.finalize().into()
    }
}

/// Structural hash used by RISC Zero for claim-like structs.
///
/// `sha256(sha256(tag) || down_0 || .. || down_n || data_le || down_count_le)`
/// where `data` words are little-endian `u32` and `down_count` is a
/// little-endian `u16`.
pub fn tagged_struct<H: Sha256>(sha: H, tag: &str, down: &[Digest], data: &[u32]) -> Digest {
    let mut buf = Vec::with_capacity(32 * (down.len() + 1) + 4 * data.len() + 2);
    buf.extend_from_slice(&sha.hash(tag.as_bytes()));
    for digest in down {
        buf.extend_from_slice(digest);
    }
    for word in data {
        buf.extend_from_slice(&word.to_le_bytes());
    }
    let count = down.len() as u16;
    buf.extend_from_slice(&count.to_le_bytes());
    sha.hash(&buf)
}

/// Digest of a `SystemState`.
pub fn system_state_digest<H: Sha256>(sha: H, merkle_root: &Digest, pc: u32) -> Digest {
    tagged_struct(sha, "risc0.SystemState", &[*merkle_root], &[pc])
}

/// Digest of the post state of a program that halted normally.
pub fn halted_post_state_digest<H: Sha256>(sha: H) -> Digest {
    system_state_digest(sha, &ZERO_DIGEST, 0)
}

/// Digest of an `Output` with no assumptions.
pub fn output_digest<H: Sha256>(sha: H, journal_digest: &Digest) -> Digest {
    // An empty assumptions list hashes to the zero digest.
    tagged_struct(sha, "risc0.Output", &[*journal_digest, ZERO_DIGEST], &[])
}

/// Digest of the journal bytes.
pub fn journal_digest<H: Sha256>(sha: H, journal: &[u8]) -> Digest {
    sha.hash(journal)
}

/// Digest of `ReceiptClaim::ok(image_id, journal)`: a program that ran to a
/// normal halt with exit code 0, no input and no assumptions.
pub fn receipt_claim_ok_digest<H: Sha256>(sha: H, image_id: &Digest, journal: &[u8]) -> Digest {
    let output = output_digest(sha, &journal_digest(sha, journal));
    tagged_struct(
        sha,
        "risc0.ReceiptClaim",
        &[
            ZERO_DIGEST,                   // input, absent
            *image_id,                     // pre state, pruned to the image id
            halted_post_state_digest(sha), // post state
            output,
        ],
        &[0, 0], // ExitCode::Halted(0) as (system, user)
    )
}

/// Split a digest into the two 128-bit field elements the Groth16 verifier
/// takes as public inputs.
///
/// The digest is reversed byte by byte, then split: the first returned value is
/// the low 128 bits, the second is the high 128 bits. This matches
/// `splitDigest` in RISC Zero's Solidity verifier, which the on-chain verifier
/// must reproduce exactly.
pub fn split_digest(digest: &Digest) -> ([u8; 16], [u8; 16]) {
    let mut reversed = [0u8; 32];
    for (i, byte) in digest.iter().rev().enumerate() {
        reversed[i] = *byte;
    }
    let mut hi = [0u8; 16];
    let mut lo = [0u8; 16];
    hi.copy_from_slice(&reversed[0..16]);
    lo.copy_from_slice(&reversed[16..32]);
    (lo, hi)
}

/// Zero-extend a 128-bit value to a 32-byte big-endian scalar.
pub fn scalar_from_u128_be(value: &[u8; 16]) -> Digest {
    let mut out = [0u8; 32];
    out[16..32].copy_from_slice(value);
    out
}

/// The five public inputs of a RISC Zero Groth16 receipt, each a 32-byte
/// big-endian BN254 scalar.
///
/// The control root and the claim digest are split into 128-bit halves because
/// a full digest does not fit in the scalar field. The BN254 control id does
/// fit, but it is stored little-endian, so it is byte-reversed here: RISC Zero
/// does the same in `risc0_groth16::Verifier::new` before parsing it as a field
/// element. Feeding it unreversed makes every pairing check fail.
pub fn groth16_public_inputs(
    control_root: &Digest,
    claim_digest: &Digest,
    bn254_control_id: &Digest,
) -> [Digest; 5] {
    let (root_lo, root_hi) = split_digest(control_root);
    let (claim_lo, claim_hi) = split_digest(claim_digest);
    [
        scalar_from_u128_be(&root_lo),
        scalar_from_u128_be(&root_hi),
        scalar_from_u128_be(&claim_lo),
        scalar_from_u128_be(&claim_hi),
        reverse_bytes(bn254_control_id),
    ]
}

/// Reverse a digest byte by byte (little-endian to big-endian and back).
pub fn reverse_bytes(digest: &Digest) -> Digest {
    let mut out = [0u8; 32];
    for (i, byte) in digest.iter().rev().enumerate() {
        out[i] = *byte;
    }
    out
}

/// The 4-byte selector that prefixes a seal, taken from the digest of the
/// Groth16 verifier parameters.
pub fn selector_from_params_digest(params_digest: &Digest) -> [u8; 4] {
    [
        params_digest[0],
        params_digest[1],
        params_digest[2],
        params_digest[3],
    ]
}

/// Digest of `Groth16ReceiptVerifierParameters`, from which the seal selector
/// is derived.
pub fn groth16_verifier_parameters_digest<H: Sha256>(
    sha: H,
    control_root: &Digest,
    bn254_control_id: &Digest,
    verifying_key_digest: &Digest,
) -> Digest {
    tagged_struct(
        sha,
        "risc0.Groth16ReceiptVerifierParameters",
        &[*control_root, *bn254_control_id, *verifying_key_digest],
        &[],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every test here hashes with `sha2`; the contract injects the host
    /// function instead and must produce the same digests.
    fn sha256(bytes: &[u8]) -> Digest {
        Sha2Hasher.hash(bytes)
    }

    #[test]
    fn tagged_struct_matches_manual_construction() {
        let down = [[1u8; 32], [2u8; 32]];
        let data = [7u32, 9u32];

        let mut expected = Vec::new();
        expected.extend_from_slice(&sha256(b"tag"));
        expected.extend_from_slice(&down[0]);
        expected.extend_from_slice(&down[1]);
        expected.extend_from_slice(&7u32.to_le_bytes());
        expected.extend_from_slice(&9u32.to_le_bytes());
        expected.extend_from_slice(&2u16.to_le_bytes());

        assert_eq!(
            tagged_struct(Sha2Hasher, "tag", &down, &data),
            sha256(&expected)
        );
    }

    #[test]
    fn split_digest_reverses_then_splits() {
        let mut digest = [0u8; 32];
        for (i, byte) in digest.iter_mut().enumerate() {
            *byte = i as u8;
        }
        let (lo, hi) = split_digest(&digest);
        // Reversed digest starts with 31, 30, ... so the high half is 31..16
        assert_eq!(hi[0], 31);
        assert_eq!(hi[15], 16);
        assert_eq!(lo[0], 15);
        assert_eq!(lo[15], 0);
    }

    #[test]
    fn scalars_are_zero_extended() {
        let value = [0xabu8; 16];
        let scalar = scalar_from_u128_be(&value);
        assert_eq!(&scalar[0..16], &[0u8; 16]);
        assert_eq!(&scalar[16..32], &value);
    }

    #[test]
    fn public_inputs_have_five_scalars_and_a_reversed_control_id() {
        let inputs = groth16_public_inputs(&[1u8; 32], &[2u8; 32], &[3u8; 32]);
        assert_eq!(inputs.len(), 5);
        // A palindromic control id survives the reversal, so use an asymmetric
        // one to prove the reversal actually happens.
        let mut control_id = [0u8; 32];
        control_id[0] = 0xaa;
        control_id[31] = 0xbb;
        let inputs = groth16_public_inputs(&[1u8; 32], &[2u8; 32], &control_id);
        assert_eq!(inputs[4][0], 0xbb);
        assert_eq!(inputs[4][31], 0xaa);
    }

    #[test]
    fn claim_digest_changes_with_journal_and_image() {
        let a = receipt_claim_ok_digest(Sha2Hasher, &[1u8; 32], b"hello");
        let b = receipt_claim_ok_digest(Sha2Hasher, &[1u8; 32], b"hell0");
        let c = receipt_claim_ok_digest(Sha2Hasher, &[2u8; 32], b"hello");
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn selector_is_the_first_four_bytes() {
        let digest = [9u8; 32];
        assert_eq!(selector_from_params_digest(&digest), [9u8, 9, 9, 9]);
    }

    #[test]
    fn a_closure_works_as_the_hasher() {
        // The contract will pass `env.crypto().sha256()` wrapped in a closure,
        // so the same digest must come out either way.
        let closure = |bytes: &[u8]| -> Digest { Sha2Hasher.hash(bytes) };
        assert_eq!(
            receipt_claim_ok_digest(closure, &[7u8; 32], b"journal"),
            receipt_claim_ok_digest(Sha2Hasher, &[7u8; 32], b"journal")
        );
    }
}
