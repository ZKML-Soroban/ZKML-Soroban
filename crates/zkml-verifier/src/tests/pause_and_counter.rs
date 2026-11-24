//! The pause switch and the verification counter, exercised with a proof that
//! really passes the pairing check.
//!
//! Both behaviours were implemented but only reachable through the accept
//! path, so a test that stops at `ContractNotInitialized` or at a malformed
//! proof never reaches them. These use the accept fixture from `test_budget`
//! so the only thing standing between the call and a recorded result is the
//! behaviour under test.

use super::test_budget::{compute_valid_proof, create_accept_fixture_vk};
use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;

/// The 80-byte public input layout: model(32) + input(32) + output(8) + class_label(8).
fn public_inputs(env: &Env, model: u8, input: u8, output: u8, class_label: u8) -> Bytes {
    let mut buf = [0u8; 80];
    buf[0] = model;
    buf[32] = input;
    buf[64] = output;
    buf[72] = class_label;
    Bytes::from_slice(env, &buf)
}

fn model_hash(env: &Env, model: u8) -> Bytes {
    let mut buf = [0u8; 32];
    buf[0] = model;
    Bytes::from_slice(env, &buf)
}

#[test]
fn paused_contract_rejects_and_records_nothing() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let vk = create_accept_fixture_vk(&env);

    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash(&env, 3), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    client.set_pause(&true);

    // A proof that would be accepted if the contract were running.
    let (proof_a, proof_b, proof_c) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);
    let inputs = public_inputs(&env, 3, 5, 42, 7);

    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &inputs);

    assert_eq!(
        result,
        Err(Ok(VerificationError::ContractPaused)),
        "a paused contract must refuse a proof it would otherwise accept"
    );
    assert_eq!(
        client.get_verification_count(),
        0,
        "a refused verification must not move the counter"
    );
    assert_eq!(
        client.get_result(),
        None,
        "a refused verification must not record a result"
    );
}

#[test]
fn counter_counts_only_successful_verifications() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let vk = create_accept_fixture_vk(&env);

    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash(&env, 3), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    let (a1, b1, c1) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);
    assert_eq!(
        client.try_verify_inference(&a1, &b1, &c1, &public_inputs(&env, 3, 5, 42, 7)),
        Ok(Ok(()))
    );
    assert_eq!(client.get_verification_count(), 1);

    // While paused, an otherwise valid proof leaves the counter alone.
    client.set_pause(&true);
    let (a2, b2, c2) = compute_valid_proof(&env, &vk, 3, 5, 43, 7);
    assert_eq!(
        client.try_verify_inference(&a2, &b2, &c2, &public_inputs(&env, 3, 5, 43, 7)),
        Err(Ok(VerificationError::ContractPaused))
    );
    assert_eq!(client.get_verification_count(), 1);

    // Unpaused, the same proof goes through. The output differs from the
    // first run because identical public inputs hash to the same nullifier
    // and would come back as ProofAlreadyUsed.
    client.set_pause(&false);
    assert_eq!(
        client.try_verify_inference(&a2, &b2, &c2, &public_inputs(&env, 3, 5, 43, 7)),
        Ok(Ok(()))
    );
    assert_eq!(client.get_verification_count(), 2);
}

/// `verify_inference` checks its proof points before the pairing, as
/// `verify_receipt` does, so a corrupted point is a typed error naming the
/// point instead of a host trap.
#[test]
fn each_corrupted_proof_point_reports_its_own_error() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let vk = create_accept_fixture_vk(&env);

    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash(&env, 3), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let (a, b, c) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);
    let inputs = public_inputs(&env, 3, 5, 42, 7);

    let flip = |point: &Bytes, pos: u32, value: Option<u8>| -> Bytes {
        let mut raw = std::vec![0u8; point.len() as usize];
        point.copy_into_slice(&mut raw);
        match value {
            Some(v) => raw[pos as usize] = v,
            None => raw[pos as usize] ^= 0x01,
        }
        Bytes::from_slice(&env, &raw)
    };

    // One flipped bit lands off the curve; 0xff in the top byte puts the
    // coordinate above the field modulus, which the host would trap on.
    for value in [None, Some(0xff)] {
        let pos = if value.is_some() { 0 } else { 40 };
        assert_eq!(
            client.try_verify_inference(&flip(&a, pos, value), &b, &c, &inputs),
            Err(Ok(VerificationError::MalformedProofA)),
            "proof_a, {value:?}"
        );
        assert_eq!(
            client.try_verify_inference(&a, &flip(&b, pos, value), &c, &inputs),
            Err(Ok(VerificationError::MalformedProofB)),
            "proof_b, {value:?}"
        );
        assert_eq!(
            client.try_verify_inference(&a, &b, &flip(&c, pos, value), &inputs),
            Err(Ok(VerificationError::MalformedProofC)),
            "proof_c, {value:?}"
        );
    }

    // Nothing was recorded, and the untouched proof still verifies.
    assert_eq!(client.get_verification_count(), 0);
    assert_eq!(client.try_verify_inference(&a, &b, &c, &inputs), Ok(Ok(())));
}

#[test]
fn a_replay_is_refused_and_leaves_the_counter_alone() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let vk = create_accept_fixture_vk(&env);

    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash(&env, 3), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    let (a, b, c) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);
    let inputs = public_inputs(&env, 3, 5, 42, 7);

    assert_eq!(client.try_verify_inference(&a, &b, &c, &inputs), Ok(Ok(())));
    assert_eq!(client.get_verification_count(), 1);

    // Same public inputs hash to the same nullifier, which is what step 9
    // of the documented order refuses.
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &inputs),
        Err(Ok(VerificationError::ProofAlreadyUsed)),
        "the second submission of the same public inputs is a replay"
    );
    assert_eq!(
        client.get_verification_count(),
        1,
        "a refused replay must not move the counter"
    );
}
