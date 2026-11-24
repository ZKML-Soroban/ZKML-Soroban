//! The commitments `zkml-common` produces are the values this contract stores
//! and compares. A model commitment is a full 254-bit scalar, little-endian,
//! so these tests run real ones through the constructor and `verify_inference`
//! instead of the small scalars the other tests use.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;
use zkml_common::commitment::{commit_inputs, commit_model, Opening};
use zkml_common::fixed_point::FixedPoint;
use zkml_common::models::{LogisticRegression, Model};

const OPENING: Opening = [0x5a; 32];

fn model() -> Model {
    Model::LogisticRegression(LogisticRegression {
        weights: std::vec![
            FixedPoint::from_raw(32_768, 16),
            FixedPoint::from_raw(-13_107, 16),
        ],
        bias: FixedPoint::from_raw(6_554, 16),
        decision_threshold: FixedPoint::from_raw(32_768, 16),
    })
}

/// The 80-byte public inputs: the two commitments as `zkml-common` returns
/// them, then output and class label as little-endian `i64`.
fn public_inputs(env: &Env, output: i64, class_label: i64) -> Bytes {
    let features = [
        FixedPoint::from_raw(39_322, 16),
        FixedPoint::from_raw(26_214, 16),
    ];
    let mut inputs = [0u8; 80];
    inputs[..32].copy_from_slice(&commit_model(&model(), &OPENING));
    inputs[32..64].copy_from_slice(&commit_inputs(&features, &OPENING));
    inputs[64..72].copy_from_slice(&output.to_le_bytes());
    inputs[72..].copy_from_slice(&class_label.to_le_bytes());
    Bytes::from_slice(env, &inputs)
}

#[test]
fn a_proof_over_real_commitments_verifies_and_records_the_model_commitment() {
    let env = Env::default();
    let inputs = public_inputs(&env, 34_865, 1);
    let model_hash = inputs.slice(0..32);
    let vk = crate::test_budget::create_accept_fixture_vk(&env);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            model_hash.clone(),
            Some(vk.clone()),
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let (a, b, c) = crate::test_budget::compute_valid_proof_for(&env, &vk, &inputs);

    assert_eq!(client.try_verify_inference(&a, &b, &c, &inputs), Ok(Ok(())));
    let record = client.get_result().expect("a verified proof is recorded");
    assert_eq!(record.model_hash, model_hash);
    assert_eq!(record.class_label, 1);
}

#[test]
fn a_commitment_under_another_opening_is_another_model() {
    let env = Env::default();
    let inputs = public_inputs(&env, 34_865, 1);
    let vk = crate::test_budget::create_accept_fixture_vk(&env);
    env.mock_all_auths();
    // Registered under a different opening: same model, different commitment.
    let other = Bytes::from_slice(&env, &commit_model(&model(), &[0xa5; 32]));
    let contract_id = env.register(
        ZkmlVerifierContract,
        (Address::generate(&env), other, Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let (a, b, c) = crate::test_budget::compute_valid_proof_for(&env, &vk, &inputs);

    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &inputs),
        Err(Ok(VerificationError::VerificationFailed))
    );
    assert_eq!(client.get_verification_count(), 0);
}
