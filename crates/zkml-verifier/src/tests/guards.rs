use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;
use test_utils::create_dummy_vk;

fn setup(env: &Env) -> ZkmlVerifierContractClient<'_> {
    let admin = Address::generate(env);
    let model_hash = Bytes::from_slice(env, &[3u8; 32]);
    let vk = create_dummy_vk(env, 5);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(env, &contract_id);
    client
}

#[test]
fn verify_malformed_proof_a() {
    let env = Env::default();
    let client = setup(&env);

    let proof_a = Bytes::from_slice(&env, &[0u8; 8]); // Wrong length
    let proof_b = Bytes::from_slice(&env, &[0u8; 128]);
    let proof_c = Bytes::from_slice(&env, &[0u8; 64]);
    let public_inputs = Bytes::from_slice(&env, &[7u8; 80]);

    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs);
    assert_eq!(result, Err(Ok(VerificationError::MalformedProofA)));
}

#[test]
fn verify_malformed_proof_b() {
    let env = Env::default();
    let client = setup(&env);

    // Use valid G1 generator point for proof_a and proof_c
    let g1_bytes: [u8; 64] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 2,
    ];
    let proof_a = Bytes::from_slice(&env, &g1_bytes);
    let proof_b = Bytes::from_slice(&env, &[0u8; 8]); // Wrong length
    let proof_c = Bytes::from_slice(&env, &g1_bytes);
    let public_inputs = Bytes::from_slice(&env, &[7u8; 80]);

    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs);
    assert_eq!(result, Err(Ok(VerificationError::MalformedProofB)));
}

#[test]
fn verify_malformed_proof_c() {
    let env = Env::default();
    let client = setup(&env);

    // Use valid G1 generator point for proof_a
    let g1_bytes: [u8; 64] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 2,
    ];
    let proof_a = Bytes::from_slice(&env, &g1_bytes);
    let proof_b = Bytes::from_slice(&env, &[0u8; 128]);
    let proof_c = Bytes::from_slice(&env, &[0u8; 8]); // Wrong length
    let public_inputs = Bytes::from_slice(&env, &[7u8; 80]);
    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs);
    // Up to VERSION 9 this was reported as MalformedProofA.
    assert_eq!(result, Err(Ok(VerificationError::MalformedProofC)));
}

#[test]
fn verify_public_inputs_too_short() {
    let env = Env::default();
    let client = setup(&env);

    let proof_a = Bytes::from_slice(&env, &[0u8; 64]);
    let proof_b = Bytes::from_slice(&env, &[0u8; 128]);
    let proof_c = Bytes::from_slice(&env, &[0u8; 64]);
    let public_inputs = Bytes::from_slice(&env, &[7u8; 32]); // Too short

    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs);
    assert_eq!(result, Err(Ok(VerificationError::PublicInputsTooShort)));
}

#[test]
fn verify_invalid_output_length() {
    let env = Env::default();
    let client = setup(&env);

    let proof_a = Bytes::from_slice(&env, &[0u8; 64]);
    let proof_b = Bytes::from_slice(&env, &[0u8; 128]);
    let proof_c = Bytes::from_slice(&env, &[0u8; 64]);
    // 32+32+8+8+8 = 88 bytes - extra 8 bytes after parsing all fields
    let public_inputs = Bytes::from_slice(&env, &[7u8; 88]);

    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs);
    assert_eq!(result, Err(Ok(VerificationError::InvalidPublicInputLength)));
}

#[test]
#[should_panic(expected = "a native verifying key has five ic points")]
fn a_native_key_without_five_ic_points_is_refused_at_deploy() {
    let env = Env::default();
    env.mock_all_auths();
    env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            Bytes::from_slice(&env, &[3u8; 32]),
            Some(create_dummy_vk(&env, 6)),
        ),
    );
}

/// The test key every contract test used to deploy with: beta, gamma and
/// delta at infinity. Against it an all-zero proof verified any public
/// input, so anyone could record a result.
fn degenerate_vk(env: &Env) -> VerificationKey {
    let mut vk = create_dummy_vk(env, 5);
    vk.beta = Bytes::from_slice(env, &[0u8; 128]);
    vk.gamma = Bytes::from_slice(env, &[0u8; 128]);
    vk.delta = Bytes::from_slice(env, &[0u8; 128]);
    vk
}

#[test]
#[should_panic(
    expected = "the native verifying key holds a point that is off the curve or at infinity"
)]
fn a_native_key_with_points_at_infinity_is_refused_at_deploy() {
    let env = Env::default();
    env.mock_all_auths();
    env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            Bytes::from_slice(&env, &[3u8; 32]),
            Some(degenerate_vk(&env)),
        ),
    );
}

#[test]
fn set_verification_key_refuses_a_degenerate_key() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            Bytes::from_slice(&env, &[3u8; 32]),
            None::<VerificationKey>,
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    assert!(client
        .try_set_verification_key(&degenerate_vk(&env))
        .is_err());
    assert!(client
        .try_set_verification_key(&create_dummy_vk(&env, 4))
        .is_err());
    // A well-formed key is still accepted.
    client.set_verification_key(&create_dummy_vk(&env, 5));
}

/// The deployment a receipt-only verifier uses: no native key. The all-zero
/// proof that forged results against the degenerate key gets a typed error.
#[test]
fn without_a_native_key_verify_inference_is_off() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            Bytes::from_slice(&env, &[3u8; 32]),
            None::<VerificationKey>,
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let (a, b, c) = test_utils::dummy_proof(&env);
    let mut inputs = [0u8; 80];
    inputs[0] = 3;
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &Bytes::from_slice(&env, &inputs)),
        Err(Ok(VerificationError::NativeKeyNotConfigured))
    );
    assert_eq!(client.get_verification_count(), 0);
}

/// The 80-byte public inputs for small scalars, each little-endian:
/// model and input hashes (32 bytes), output and class label (8 bytes).
fn scalar_inputs(env: &Env, model: u8, input: u8, output: u8, class_label: u8) -> Bytes {
    let mut inputs = [0u8; 80];
    inputs[0] = model;
    inputs[32] = input;
    inputs[64] = output;
    inputs[72] = class_label;
    Bytes::from_slice(env, &inputs)
}

/// A contract registered for model 3 against the accept-path key, so a proof
/// from `compute_valid_proof` verifies and nothing but the behavior under
/// test can make a call fail.
fn accept_setup(env: &Env) -> (ZkmlVerifierContractClient<'_>, VerificationKey) {
    let vk = crate::test_budget::create_accept_fixture_vk(env);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(env),
            scalar_inputs(env, 3, 0, 0, 0).slice(0..32),
            Some(vk.clone()),
        ),
    );
    (ZkmlVerifierContractClient::new(env, &contract_id), vk)
}

/// Four public inputs (model, input, output, class label) against a key with
/// five `ic` points verify, and the record carries the output and the label.
/// This used to send an all-zero proof and only check that the error was not
/// `VerificationKeyLengthMismatch`, so it passed whatever happened.
#[test]
fn verify_multi_scalar_output() {
    let env = Env::default();
    let (client, vk) = accept_setup(&env);
    let (a, b, c) = crate::test_budget::compute_valid_proof(&env, &vk, 3, 5, 42, 7);

    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &scalar_inputs(&env, 3, 5, 42, 7)),
        Ok(Ok(()))
    );
    let record = client.get_result().expect("a verified proof is recorded");
    assert_eq!(
        record.output,
        scalar_inputs(&env, 3, 5, 42, 7).slice(64..72)
    );
    assert_eq!(record.class_label, 7);
}

#[test]
fn verify_wrong_model_hash() {
    let env = Env::default();

    // Initialize with model hash [3u8; 32]
    let admin = Address::generate(&env);
    let model_hash = Bytes::from_slice(&env, &[3u8; 32]);
    let vk = create_dummy_vk(&env, 5);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    // Try to verify with different model hash [5u8; 32]
    let proof_a = Bytes::from_slice(&env, &[0u8; 64]);
    let proof_b = Bytes::from_slice(&env, &[0u8; 128]);
    let proof_c = Bytes::from_slice(&env, &[0u8; 64]);
    let public_inputs = Bytes::from_slice(&env, &[5u8; 80]); // 32+32+8+8 = 80 bytes for single output + class_label

    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs);
    assert_eq!(result, Err(Ok(VerificationError::VerificationFailed)));
}

#[test]
fn verify_replay_attack_prevented() {
    let env = Env::default();

    // A real proof against the accept-path key: model 3, input 5, output
    // 42, label 7, each little-endian in the 80-byte layout. This used to
    // run an all-zero proof against a degenerate key, which only passed
    // because that key accepted anything.
    let mut inputs = [0u8; 80];
    inputs[0] = 3;
    inputs[32] = 5;
    inputs[64] = 42;
    inputs[72] = 7;
    let public_inputs = Bytes::from_slice(&env, &inputs);
    let vk = crate::test_budget::create_accept_fixture_vk(&env);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            public_inputs.slice(0..32),
            Some(vk.clone()),
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let (proof_a, proof_b, proof_c) =
        crate::test_budget::compute_valid_proof(&env, &vk, 3, 5, 42, 7);

    assert_eq!(
        client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs),
        Ok(Ok(()))
    );
    // The same proof and public inputs a second time is a replay.
    assert_eq!(
        client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs),
        Err(Ok(VerificationError::ProofAlreadyUsed))
    );
    assert_eq!(client.get_verification_count(), 1);
}

/// Two proofs that differ only in the input hash are two inferences: both are
/// recorded, and each is still refused as a replay afterwards. These used to
/// send 72-byte inputs, which `parse_public_inputs` refused before the
/// nullifier was ever read, so the "not `ProofAlreadyUsed`" checks could not
/// fail.
#[test]
fn verify_distinct_inputs_independent() {
    let env = Env::default();
    let (client, vk) = accept_setup(&env);
    let (a1, b1, c1) = crate::test_budget::compute_valid_proof(&env, &vk, 3, 5, 42, 7);
    let (a2, b2, c2) = crate::test_budget::compute_valid_proof(&env, &vk, 3, 6, 42, 7);
    let first = scalar_inputs(&env, 3, 5, 42, 7);
    let second = scalar_inputs(&env, 3, 6, 42, 7);

    assert_eq!(
        client.try_verify_inference(&a1, &b1, &c1, &first),
        Ok(Ok(()))
    );
    assert_eq!(
        client.try_verify_inference(&a2, &b2, &c2, &second),
        Ok(Ok(()))
    );
    assert_eq!(client.get_verification_count(), 2);
    assert_eq!(
        client.try_verify_inference(&a2, &b2, &c2, &second),
        Err(Ok(VerificationError::ProofAlreadyUsed))
    );
}

/// The same for two proofs that differ only in the output.
#[test]
fn verify_distinct_outputs_independent() {
    let env = Env::default();
    let (client, vk) = accept_setup(&env);
    let (a1, b1, c1) = crate::test_budget::compute_valid_proof(&env, &vk, 3, 5, 42, 7);
    let (a2, b2, c2) = crate::test_budget::compute_valid_proof(&env, &vk, 3, 5, 43, 7);
    let first = scalar_inputs(&env, 3, 5, 42, 7);
    let second = scalar_inputs(&env, 3, 5, 43, 7);

    assert_eq!(
        client.try_verify_inference(&a1, &b1, &c1, &first),
        Ok(Ok(()))
    );
    assert_eq!(
        client.try_verify_inference(&a2, &b2, &c2, &second),
        Ok(Ok(()))
    );
    assert_eq!(client.get_verification_count(), 2);
    assert_eq!(
        client.try_verify_inference(&a1, &b1, &c1, &first),
        Err(Ok(VerificationError::ProofAlreadyUsed))
    );
}
