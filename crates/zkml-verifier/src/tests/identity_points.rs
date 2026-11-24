//! Proof points at infinity.
//!
//! Soroban encodes the point at infinity as all zero bytes, and the curve
//! checks accept that encoding: the host's `g1_is_on_curve` and the contract's
//! own G2 check both say yes. An honest Groth16 proof never holds the identity,
//! and a proof that does removes a term from the pairing, so since VERSION 11
//! both verification paths refuse it before any cryptography.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;

fn inputs(env: &Env) -> Bytes {
    let mut inputs = [0u8; 80];
    inputs[0] = 3;
    inputs[32] = 5;
    inputs[64] = 42;
    inputs[72] = 7;
    Bytes::from_slice(env, &inputs)
}

fn setup(env: &Env) -> (ZkmlVerifierContractClient<'_>, (Bytes, Bytes, Bytes)) {
    let vk = crate::test_budget::create_accept_fixture_vk(env);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(env),
            inputs(env).slice(0..32),
            Some(vk.clone()),
        ),
    );
    let proof = crate::test_budget::compute_valid_proof(env, &vk, 3, 5, 42, 7);
    (ZkmlVerifierContractClient::new(env, &contract_id), proof)
}

#[test]
fn the_curve_checks_accept_the_all_zero_encoding() {
    // Pins why the explicit identity check is needed: neither check below
    // rejects infinity on its own.
    let env = Env::default();
    let zero = Bn254G1Affine::from_array(&env, &[0u8; 64]);
    assert!(env.crypto().bn254().g1_is_on_curve(&zero));
    assert!(zkml_common::bn254::g2_is_on_curve(&[0u8; 128]));
}

#[test]
fn verify_inference_refuses_each_point_at_infinity() {
    let env = Env::default();
    let (client, (a, b, c)) = setup(&env);
    let zero_g1 = Bytes::from_slice(&env, &[0u8; 64]);
    let zero_g2 = Bytes::from_slice(&env, &[0u8; 128]);

    assert_eq!(
        client.try_verify_inference(&zero_g1, &b, &c, &inputs(&env)),
        Err(Ok(VerificationError::MalformedProofA))
    );
    assert_eq!(
        client.try_verify_inference(&a, &zero_g2, &c, &inputs(&env)),
        Err(Ok(VerificationError::MalformedProofB))
    );
    assert_eq!(
        client.try_verify_inference(&a, &b, &zero_g1, &inputs(&env)),
        Err(Ok(VerificationError::MalformedProofC))
    );
    assert_eq!(client.get_verification_count(), 0);
    // The untouched proof still verifies, so only the zeroed point failed.
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &inputs(&env)),
        Ok(Ok(()))
    );
}
