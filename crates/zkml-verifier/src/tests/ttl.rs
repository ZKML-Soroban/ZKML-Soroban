//! The instance TTL policy in `docs/reference/verifier-contract.md`.
//!
//! The constructor, both verification entry points on success, the RISC Zero
//! setters and both steps of the admin transfer renew the instance TTL; the
//! other admin setters and a failed verification do not.

use super::*;
use soroban_sdk::testutils::{storage::Instance as _, Address as _, Ledger as _};
use soroban_sdk::Env;

fn instance_ttl(env: &Env, contract_id: &Address) -> u32 {
    env.as_contract(contract_id, || env.storage().instance().get_ttl())
}

fn advance_ledger(env: &Env, ledgers: u32) {
    let mut ledger = env.ledger().get();
    ledger.sequence_number = ledger.sequence_number.saturating_add(ledgers);
    env.ledger().set(ledger);
}

/// Ledgers to advance so the remaining TTL is below the renewal threshold
/// but the instance has not expired.
const BELOW_THRESHOLD: u32 = INSTANCE_TTL_EXTEND_TO - INSTANCE_TTL_THRESHOLD + 10;

/// The 80-byte public input for model 3, input 5, `output`, label 7.
fn public_inputs(env: &Env, output: u8) -> Bytes {
    let mut inputs = [0u8; 80];
    inputs[0] = 3;
    inputs[32] = 5;
    inputs[64] = output;
    inputs[72] = 7;
    Bytes::from_slice(env, &inputs)
}

/// A contract whose native key accepts a real proof of output 42.
fn setup(env: &Env) -> (Address, ZkmlVerifierContractClient<'_>, VerificationKey) {
    env.mock_all_auths();
    let vk = crate::test_budget::create_accept_fixture_vk(env);
    let model_hash = public_inputs(env, 42).slice(0..32);
    let contract_id = env.register(
        ZkmlVerifierContract,
        (Address::generate(env), model_hash, Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(env, &contract_id);
    (contract_id, client, vk)
}

fn valid_proof(env: &Env, vk: &VerificationKey) -> (Bytes, Bytes, Bytes) {
    crate::test_budget::compute_valid_proof(env, vk, 3, 5, 42, 7)
}

#[test]
fn constructor_extends_instance_ttl() {
    let env = Env::default();
    let (contract_id, _client, _vk) = setup(&env);
    let ttl = instance_ttl(&env, &contract_id);
    assert!(
        ttl >= INSTANCE_TTL_EXTEND_TO,
        "instance TTL after deploy was {ttl}, expected at least {INSTANCE_TTL_EXTEND_TO}"
    );
}

#[test]
fn successful_verify_inference_renews_instance_ttl() {
    let env = Env::default();
    let (contract_id, client, vk) = setup(&env);

    advance_ledger(&env, BELOW_THRESHOLD);
    let before = instance_ttl(&env, &contract_id);
    assert!(
        before < INSTANCE_TTL_THRESHOLD,
        "precondition: remaining TTL {before} should be below the threshold"
    );

    let (a, b, c) = valid_proof(&env, &vk);
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &public_inputs(&env, 42)),
        Ok(Ok(()))
    );
    let after = instance_ttl(&env, &contract_id);
    assert!(
        after >= INSTANCE_TTL_EXTEND_TO,
        "a successful verification should renew the TTL to at least \
         {INSTANCE_TTL_EXTEND_TO}, got {after}"
    );
    assert_eq!(client.get_verification_count(), 1);
}

#[test]
fn failed_verify_inference_does_not_renew_instance_ttl() {
    let env = Env::default();
    let (contract_id, client, vk) = setup(&env);

    advance_ledger(&env, BELOW_THRESHOLD);
    let before = instance_ttl(&env, &contract_id);

    // A real proof of output 42, submitted as output 43: the pairing fails.
    let (a, b, c) = valid_proof(&env, &vk);
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &public_inputs(&env, 43)),
        Err(Ok(VerificationError::VerificationFailed))
    );
    assert_eq!(instance_ttl(&env, &contract_id), before);
    assert_eq!(client.get_verification_count(), 0);
}

#[test]
fn state_survives_past_the_threshold_after_a_verification() {
    let env = Env::default();
    let (contract_id, client, vk) = setup(&env);

    // Past the 30-day threshold, then a verification renews the instance.
    advance_ledger(&env, INSTANCE_TTL_THRESHOLD + 1);
    let (a, b, c) = valid_proof(&env, &vk);
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &public_inputs(&env, 42)),
        Ok(Ok(()))
    );

    // Another threshold later, the state written before is still there.
    advance_ledger(&env, INSTANCE_TTL_THRESHOLD + 1);
    assert_eq!(
        client.get_model_hash(),
        Some(public_inputs(&env, 42).slice(0..32))
    );
    assert_eq!(client.get_verification_count(), 1);
    assert!(client.get_result().is_some());
    assert!(instance_ttl(&env, &contract_id) > 0);
}

#[test]
fn failed_verify_leaves_state_readable() {
    let env = Env::default();
    let (_contract_id, client, vk) = setup(&env);
    let (a, b, c) = valid_proof(&env, &vk);
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &public_inputs(&env, 43)),
        Err(Ok(VerificationError::VerificationFailed))
    );
    assert_eq!(
        client.get_model_hash(),
        Some(public_inputs(&env, 42).slice(0..32))
    );
    assert_eq!(client.get_verification_count(), 0);
    assert!(client.get_result().is_none());
}

/// TTL after running `call` once the remaining TTL is below the threshold.
fn ttl_change(call: impl FnOnce(&Env, &ZkmlVerifierContractClient<'_>)) -> (u32, u32) {
    let env = Env::default();
    let (contract_id, client, _vk) = setup(&env);
    advance_ledger(&env, BELOW_THRESHOLD);
    let before = instance_ttl(&env, &contract_id);
    call(&env, &client);
    (before, instance_ttl(&env, &contract_id))
}

#[test]
fn admin_transfer_renews_instance_ttl_at_each_step() {
    let (before, after) = ttl_change(|env, client| {
        client.propose_admin(&Address::generate(env));
    });
    assert!(before < INSTANCE_TTL_THRESHOLD);
    assert!(
        after >= INSTANCE_TTL_EXTEND_TO,
        "propose_admin left TTL at {after}"
    );

    let env = Env::default();
    let (contract_id, client, _vk) = setup(&env);
    client.propose_admin(&Address::generate(&env));
    advance_ledger(&env, BELOW_THRESHOLD);
    assert!(instance_ttl(&env, &contract_id) < INSTANCE_TTL_THRESHOLD);
    client.accept_admin();
    let after = instance_ttl(&env, &contract_id);
    assert!(
        after >= INSTANCE_TTL_EXTEND_TO,
        "accept_admin left TTL at {after}"
    );
}

fn hex(s: &str) -> std::vec::Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn fixture(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("json")
}

fn bytes32(env: &Env, value: &serde_json::Value) -> BytesN<32> {
    let array: [u8; 32] = hex(value.as_str().unwrap()).try_into().expect("32 bytes");
    BytesN::from_array(env, &array)
}

/// The golden fixture's RISC Zero configuration and verifying key.
fn risc0_setup(env: &Env) -> (Risc0Config, VerificationKey) {
    let bundle = fixture("groth16_bundle.json");
    let params = fixture("risc0_vk.json");
    let selector: [u8; 4] = hex(params["selector"].as_str().unwrap())
        .try_into()
        .expect("4 bytes");
    let config = Risc0Config {
        image_id: bytes32(env, &bundle["image_id"]),
        control_root: bytes32(env, &params["control_root"]),
        bn254_control_id: bytes32(env, &params["bn254_control_id"]),
        selector: BytesN::from_array(env, &selector),
    };
    let vk = &params["vk"];
    let point = |name: &str| Bytes::from_slice(env, &hex(vk[name].as_str().unwrap()));
    let mut ic = Vec::new(env);
    for p in vk["ic"].as_array().unwrap() {
        ic.push_back(Bytes::from_slice(env, &hex(p.as_str().unwrap())));
    }
    let key = VerificationKey {
        alpha: point("alpha"),
        beta: point("beta"),
        gamma: point("gamma"),
        delta: point("delta"),
        ic,
    };
    (config, key)
}

#[test]
fn risc0_setters_renew_instance_ttl() {
    let (before, after) = ttl_change(|env, client| {
        client.set_risc0_config(&risc0_setup(env).0);
    });
    assert!(before < INSTANCE_TTL_THRESHOLD);
    assert!(
        after >= INSTANCE_TTL_EXTEND_TO,
        "set_risc0_config left TTL at {after}"
    );

    let (_before, after) = ttl_change(|env, client| {
        client.set_risc0_vk(&risc0_setup(env).1);
    });
    assert!(
        after >= INSTANCE_TTL_EXTEND_TO,
        "set_risc0_vk left TTL at {after}"
    );
}

#[test]
fn successful_verify_receipt_renews_instance_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    let bundle = fixture("groth16_bundle.json");
    let journal_raw = hex(bundle["journal"].as_str().unwrap());
    let decoded = zkml_common::journal::JournalV1::decode(&journal_raw).expect("journal");
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            Bytes::from_slice(&env, &decoded.model_hash),
            None::<VerificationKey>,
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let (config, key) = risc0_setup(&env);
    client.set_risc0_config(&config);
    client.set_risc0_vk(&key);

    advance_ledger(&env, BELOW_THRESHOLD);
    assert!(instance_ttl(&env, &contract_id) < INSTANCE_TTL_THRESHOLD);

    let seal = Bytes::from_slice(&env, &hex(bundle["seal"].as_str().unwrap()));
    let journal = Bytes::from_slice(&env, &journal_raw);
    assert!(client.try_verify_receipt(&seal, &journal).is_ok());
    let after = instance_ttl(&env, &contract_id);
    assert!(
        after >= INSTANCE_TTL_EXTEND_TO,
        "verify_receipt left TTL at {after}"
    );
}

#[test]
fn the_other_admin_setters_do_not_renew_instance_ttl() {
    let (before, after) = ttl_change(|env, client| {
        client.set_verification_key(&crate::test_budget::create_accept_fixture_vk(env));
    });
    assert_eq!(after, before, "set_verification_key changed the TTL");

    let (before, after) = ttl_change(|env, client| {
        client.set_model_hash(&Bytes::from_slice(env, &[9u8; 32]));
    });
    assert_eq!(after, before, "set_model_hash changed the TTL");

    let (before, after) = ttl_change(|_env, client| {
        client.set_pause(&true);
    });
    assert_eq!(after, before, "set_pause changed the TTL");
}
