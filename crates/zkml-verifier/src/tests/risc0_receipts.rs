//! The contract side of RISC Zero receipt verification.
//!
//! The value these tests protect is agreement: the contract recomputes a claim
//! digest with the Soroban host's SHA-256, while the prover computes it with
//! `sha2`. If the two ever disagree by one byte, every proof fails on chain
//! with nothing to point at. So the expected digests here are the ones
//! `zkml-common` produces, and `crates/zkml-prover/tests/risc0_digests.rs`
//! separately pins those against `risc0-zkvm` itself.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Bytes;

/// A journal the guest could really have committed.
fn journal_bytes(env: &Env) -> Bytes {
    let journal = zkml_common::journal::JournalV1 {
        model_kind: zkml_common::journal::ModelKind::LogisticRegression,
        model_hash: [0x11; 32],
        input_hash: [0x22; 32],
        output: 34865,
        class_label: 1,
    };
    Bytes::from_slice(env, &journal.encode())
}

fn setup(env: &Env) -> (ZkmlVerifierContractClient<'_>, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let model_hash = Bytes::from_slice(env, &[0x11; 32]);
    let vk = crate::test_utils::create_dummy_vk(env, 5);
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(env, &contract_id);
    (client, admin)
}

fn config(env: &Env) -> Risc0Config {
    Risc0Config {
        image_id: BytesN::from_array(env, &[0xaa; 32]),
        control_root: BytesN::from_array(env, &[0xbb; 32]),
        // Reversed it must be below r, so not something like 0xcc repeated.
        bn254_control_id: BytesN::from_array(env, &[0x01; 32]),
        selector: BytesN::from_array(env, &[0x73, 0xc4, 0x57, 0xba]),
    }
}

#[test]
fn the_contract_and_the_prover_agree_on_the_claim_digest() {
    let env = Env::default();
    let (client, _admin) = setup(&env);

    let image_id = [0xaa_u8; 32];
    let journal = journal_bytes(&env);

    // What the contract computes, with the host's SHA-256.
    let on_chain = client.claim_digest(&BytesN::from_array(&env, &image_id), &journal);

    // What the shared arithmetic computes off chain, with sha2.
    let mut raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
    journal.copy_into_slice(&mut raw);
    let off_chain = zkml_common::risc0::receipt_claim_ok_digest(
        zkml_common::risc0::Sha2Hasher,
        &image_id,
        &raw,
    );

    assert_eq!(
        on_chain,
        BytesN::from_array(&env, &off_chain),
        "the host SHA-256 and sha2 must produce the same claim digest"
    );
}

#[test]
fn the_claim_digest_binds_the_journal_and_the_image_id() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let image_id = BytesN::from_array(&env, &[0xaa; 32]);
    let journal = journal_bytes(&env);
    let base = client.claim_digest(&image_id, &journal);

    // One different byte in the journal.
    let mut raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
    journal.copy_into_slice(&mut raw);
    raw[72] ^= 0x01; // the output field
    let other_journal = Bytes::from_slice(&env, &raw);
    assert_ne!(base, client.claim_digest(&image_id, &other_journal));

    // One different byte in the image id.
    let mut id = [0xaa_u8; 32];
    id[0] ^= 0x01;
    assert_ne!(
        base,
        client.claim_digest(&BytesN::from_array(&env, &id), &journal)
    );
}

#[test]
fn the_risc0_config_round_trips() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let cfg = config(&env);
    client.set_risc0_config(&cfg);

    let stored = client.get_risc0_config();
    assert_eq!(stored.image_id, cfg.image_id);
    assert_eq!(stored.control_root, cfg.control_root);
    assert_eq!(stored.bn254_control_id, cfg.bn254_control_id);
    assert_eq!(stored.selector, cfg.selector);
}

/// The control id as it would be stored, for a scalar value given
/// big-endian: stored bytes are the reverse of the scalar's.
fn control_id_for_scalar(env: &Env, scalar_be: [u8; 32]) -> BytesN<32> {
    let mut stored = scalar_be;
    stored.reverse();
    BytesN::from_array(env, &stored)
}

#[test]
fn a_control_id_just_below_the_scalar_modulus_is_accepted() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let mut r_minus_one = BN254_SCALAR_MODULUS;
    r_minus_one[31] -= 1;
    let mut cfg = config(&env);
    cfg.bn254_control_id = control_id_for_scalar(&env, r_minus_one);
    client.set_risc0_config(&cfg);
}

#[test]
#[should_panic(expected = "bn254_control_id is not a valid BN254 scalar")]
fn a_control_id_equal_to_the_scalar_modulus_is_rejected() {
    // The host would reduce it to 0 without complaint and check it as a
    // different id.
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let mut cfg = config(&env);
    cfg.bn254_control_id = control_id_for_scalar(&env, BN254_SCALAR_MODULUS);
    client.set_risc0_config(&cfg);
}

#[test]
fn the_claim_digest_view_refuses_a_journal_of_the_wrong_length() {
    // Padding or truncating would return a digest that matches neither
    // RISC Zero nor the prover, so a short or long journal is a typed
    // error, not a trap.
    let env = Env::default();
    let (client, _admin) = setup(&env);
    let image_id = BytesN::from_array(&env, &[0xaa; 32]);
    for len in [0usize, 95, 97] {
        let journal = Bytes::from_slice(&env, &std::vec![0u8; len]);
        assert_eq!(
            client.try_claim_digest(&image_id, &journal),
            Err(Ok(VerificationError::MalformedJournal)),
            "a {len}-byte journal must be refused"
        );
    }
}

#[test]
fn changing_the_config_emits_an_event() {
    use soroban_sdk::testutils::Events;
    use soroban_sdk::IntoVal;

    let env = Env::default();
    let (client, _admin) = setup(&env);
    let cfg = config(&env);
    client.set_risc0_config(&cfg);

    assert_eq!(
        env.events().all().filter_by_contract(&client.address),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("cfg_upd"), symbol_short!("risc0")).into_val(&env),
                cfg.image_id.clone().into_val(&env),
            ),
        ],
        "set_risc0_config must publish cfg_upd so admin changes are visible"
    );
}

#[test]
#[should_panic(expected = "risc0 config has not been set")]
fn reading_the_config_before_setting_it_says_so() {
    let env = Env::default();
    let (client, _admin) = setup(&env);
    client.get_risc0_config();
}
