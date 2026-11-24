//! One inference, one nullifier, whatever the encoding.
//!
//! Each 32-byte public input reaches the pairing as a BN254 scalar, and the host
//! reduces modulo `r` silently, so `x` and `x + r` are the same scalar. The
//! nullifier is `sha256` over the raw bytes, so before `require_canonical_scalar`
//! a single valid proof could be recorded once per multiple of `r` that still fit
//! in 32 bytes: the counter reached 6 from one proof. These tests pin the rule
//! that keeps the two views in agreement.

use super::*;
use crate::test_budget::{compute_valid_proof, create_accept_fixture_vk};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;

/// `r`, the BN254 scalar field modulus, big-endian. Same value as
/// `BN254_SCALAR_MODULUS`, repeated here so the test would catch that
/// constant being changed as well.
const R_BE: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00, 0x00, 0x01,
];

/// Little-endian 32 bytes holding `value + k * r`, which the contract reads
/// as the scalar `value`.
fn field_bytes(value: u64, k: u64) -> [u8; 32] {
    let mut acc = [0u8; 32];
    acc[24..32].copy_from_slice(&value.to_be_bytes());
    for _ in 0..k {
        let mut carry = 0u16;
        for i in (0..32).rev() {
            let sum = acc[i] as u16 + R_BE[i] as u16 + carry;
            acc[i] = (sum & 0xff) as u8;
            carry = sum >> 8;
        }
    }
    let mut le = [0u8; 32];
    for i in 0..32 {
        le[i] = acc[31 - i];
    }
    le
}

fn public_inputs(
    env: &Env,
    model_le: [u8; 32],
    input_le: [u8; 32],
    output: u64,
    label: u64,
) -> Bytes {
    let mut buf = [0u8; 80];
    buf[0..32].copy_from_slice(&model_le);
    buf[32..64].copy_from_slice(&input_le);
    buf[64..72].copy_from_slice(&output.to_le_bytes());
    buf[72..80].copy_from_slice(&label.to_le_bytes());
    Bytes::from_slice(env, &buf)
}

fn setup(env: &Env) -> (ZkmlVerifierContractClient<'_>, VerificationKey) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let vk = create_accept_fixture_vk(env);
    let model_hash = Bytes::from_slice(env, &field_bytes(3, 0));
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(env, &contract_id);
    (client, vk)
}

/// The whole point: one proof cannot be re-encoded into a second record.
#[test]
fn re_encoding_the_input_hash_cannot_replay_a_proof() {
    let env = Env::default();
    let (client, vk) = setup(&env);
    let (a, b, c) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);

    // The canonical encoding is accepted once.
    assert_eq!(
        client.try_verify_inference(
            &a,
            &b,
            &c,
            &public_inputs(&env, field_bytes(3, 0), field_bytes(5, 0), 42, 7)
        ),
        Ok(Ok(()))
    );
    assert_eq!(client.get_verification_count(), 1);

    // Every non-canonical encoding of the same scalar is refused outright,
    // so it never reaches the nullifier check.
    for k in 1..=5u64 {
        assert_eq!(
            client.try_verify_inference(
                &a,
                &b,
                &c,
                &public_inputs(&env, field_bytes(3, 0), field_bytes(5, k), 42, 7)
            ),
            Err(Ok(VerificationError::NonCanonicalPublicInput)),
            "input_hash of 5 + {k}r is the same scalar and must not be a second record"
        );
    }

    // And the byte-identical resubmission is still a replay.
    assert_eq!(
        client.try_verify_inference(
            &a,
            &b,
            &c,
            &public_inputs(&env, field_bytes(3, 0), field_bytes(5, 0), 42, 7)
        ),
        Err(Ok(VerificationError::ProofAlreadyUsed))
    );

    assert_eq!(
        client.get_verification_count(),
        1,
        "one proof, one record, whatever the encoding"
    );
}

/// `model_hash` gets the same treatment, before it is compared to storage.
#[test]
fn a_non_canonical_model_hash_is_refused() {
    let env = Env::default();
    let (client, vk) = setup(&env);
    let (a, b, c) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);

    assert_eq!(
        client.try_verify_inference(
            &a,
            &b,
            &c,
            &public_inputs(&env, field_bytes(3, 1), field_bytes(5, 0), 42, 7)
        ),
        Err(Ok(VerificationError::NonCanonicalPublicInput))
    );
    assert_eq!(client.get_verification_count(), 0);
}

/// Exactly `r` is the first rejected value, and `r - 1` the last accepted
/// one, so the comparison is `>=` and not `>`.
///
/// Both cases use the valid proof, so that the canonical one gets past the
/// point checks and is refused by the pairing, not by an earlier check.
#[test]
fn the_boundary_is_r_itself() {
    let env = Env::default();
    let (client, vk) = setup(&env);
    let (a, b, c) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);

    let mut r_le = [0u8; 32];
    for i in 0..32 {
        r_le[i] = R_BE[31 - i];
    }
    assert_eq!(
        client.try_verify_inference(
            &a,
            &b,
            &c,
            &public_inputs(&env, field_bytes(3, 0), r_le, 42, 7)
        ),
        Err(Ok(VerificationError::NonCanonicalPublicInput)),
        "r is not a canonical scalar"
    );

    // r - 1 is the largest canonical value, so it gets past this check and
    // fails the pairing instead, the proof having been built for 5.
    let mut minus_one = r_le;
    minus_one[0] -= 1;
    assert_eq!(
        client.try_verify_inference(
            &a,
            &b,
            &c,
            &public_inputs(&env, field_bytes(3, 0), minus_one, 42, 7)
        ),
        Err(Ok(VerificationError::VerificationFailed)),
        "r - 1 is canonical and must reach the pairing"
    );
}
