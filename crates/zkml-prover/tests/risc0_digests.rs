//! Cross-checks `zkml_common::risc0` against the real RISC Zero crates.
//!
//! `zkml-common` reimplements RISC Zero's digest arithmetic so the `no_std`
//! verifier can recompute a claim digest without pulling in `risc0-zkvm`. A
//! reimplementation is only useful if it agrees with the original, so every
//! function that feeds the on-chain public inputs is compared here against the
//! value `risc0-zkvm` produces for the same input.
//!
//! These tests need the `zkvm` feature (that is what brings in `risc0-zkvm`);
//! they do not prove anything, so they are fast and need neither Docker nor
//! x86_64.

#![cfg(feature = "zkvm")]

use risc0_zkvm::sha::{Digest, Digestible};
use risc0_zkvm::{Groth16ReceiptVerifierParameters, MaybePruned, ReceiptClaim};

use zkml_common::risc0::{
    groth16_public_inputs, groth16_verifier_parameters_digest, journal_digest,
    receipt_claim_ok_digest, selector_from_params_digest, split_digest, Sha2Hasher, SEAL_LEN,
};

/// A handful of journals, including the empty one and the 96-byte layout the
/// guest actually commits.
fn journals() -> Vec<Vec<u8>> {
    let mut out = vec![
        Vec::new(),
        vec![0u8],
        b"zkml".to_vec(),
        (0u8..96).collect(),
        vec![0xffu8; 96],
    ];
    // And many more from a fixed pseudo-random sequence, at lengths that
    // straddle SHA-256's 55/56/64-byte padding boundaries, so a failure
    // reproduces and length-dependent bugs have somewhere to show.
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for len in (0..=200).chain([255, 256, 257, 511, 512, 1000]) {
        out.push((0..len).map(|_| next() as u8).collect());
    }
    out
}

fn image_ids() -> Vec<[u8; 32]> {
    vec![
        [0u8; 32],
        [0xffu8; 32],
        {
            let mut id = [0u8; 32];
            for (i, byte) in id.iter_mut().enumerate() {
                *byte = i as u8;
            }
            id
        },
        zkml_prover::prover::image_id(),
    ]
}

#[test]
fn journal_digest_matches_risc0() {
    for journal in journals() {
        let ours = journal_digest(Sha2Hasher, &journal);
        let theirs: [u8; 32] = risc0_zkvm::Journal::new(journal.clone()).digest().into();
        assert_eq!(ours, theirs, "journal digest differs for {journal:?}");
    }
}

#[test]
fn receipt_claim_ok_digest_matches_risc0() {
    for image_id in image_ids() {
        for journal in journals() {
            let ours = receipt_claim_ok_digest(Sha2Hasher, &image_id, &journal);

            let image: Digest = Digest::try_from(image_id).expect("32 bytes is a digest");
            let claim = ReceiptClaim::ok(image, MaybePruned::Value(journal.clone()));
            let theirs: [u8; 32] = claim.digest().into();

            assert_eq!(
                ours, theirs,
                "claim digest differs for image {image_id:?} and journal {journal:?}"
            );
        }
    }
}

#[test]
fn verifier_parameters_digest_and_selector_match_risc0() {
    let params = Groth16ReceiptVerifierParameters::default();
    let theirs: [u8; 32] = params.digest().into();

    let control_root: [u8; 32] = params.control_root.into();
    let bn254_control_id: [u8; 32] = params.bn254_control_id.into();
    let vk_digest: [u8; 32] = params.verifying_key.digest().into();

    let ours = groth16_verifier_parameters_digest(
        Sha2Hasher,
        &control_root,
        &bn254_control_id,
        &vk_digest,
    );
    assert_eq!(
        ours, theirs,
        "verifier parameters digest must match risc0-zkvm, otherwise the seal \
         selector we export is wrong"
    );

    assert_eq!(selector_from_params_digest(&ours), theirs[..4]);
}

#[test]
fn split_digest_recomposes_to_the_reversed_digest() {
    for journal in journals() {
        let digest = journal_digest(Sha2Hasher, &journal);
        let (lo, hi) = split_digest(&digest);

        let mut recomposed = [0u8; 32];
        recomposed[..16].copy_from_slice(&hi);
        recomposed[16..].copy_from_slice(&lo);
        recomposed.reverse();

        assert_eq!(recomposed, digest, "split_digest must be reversible");
    }
}

#[test]
fn groth16_public_inputs_are_five_bn254_scalars() {
    let params = Groth16ReceiptVerifierParameters::default();
    let control_root: [u8; 32] = params.control_root.into();
    let bn254_control_id: [u8; 32] = params.bn254_control_id.into();
    let claim = receipt_claim_ok_digest(Sha2Hasher, &zkml_prover::prover::image_id(), &[7u8; 96]);

    let inputs = groth16_public_inputs(&control_root, &claim, &bn254_control_id);

    // The four split halves are 128-bit values zero-extended to 32 bytes, so
    // their top half must be zero. The control id is used whole, but reversed:
    // risc0-groth16 reverses it before parsing it as a field element, and a
    // verifier that skips that step fails every pairing check.
    for (i, scalar) in inputs.iter().take(4).enumerate() {
        assert_eq!(scalar[..16], [0u8; 16], "input {i} is not 128-bit");
    }
    let mut reversed_id = bn254_control_id;
    reversed_id.reverse();
    assert_eq!(
        inputs[4], reversed_id,
        "the bn254 control id must be reversed"
    );

    // Every scalar must be below the BN254 scalar field modulus, otherwise the
    // host function that parses them would reject the proof.
    const R: [u8; 32] = [
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58,
        0x5d, 0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00,
        0x00, 0x01,
    ];
    for (i, scalar) in inputs.iter().enumerate() {
        assert!(scalar[..] < R[..], "input {i} is not reduced mod r");
    }
}

#[test]
fn seal_len_matches_a_selector_plus_a_groth16_proof() {
    // 4-byte selector + three BN254 points (A: G1, B: G2, C: G1) at 32 bytes
    // per field element.
    assert_eq!(SEAL_LEN, 4 + 2 * 32 + 4 * 32 + 2 * 32);
}
