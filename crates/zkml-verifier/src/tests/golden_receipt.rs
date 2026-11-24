//! The golden fixture: a real receipt, verified on chain.
//!
//! Everything else in this file tests the contract against values chosen to
//! exercise a path. This tests it against a proof that a real zkVM run really
//! produced, with the verifying key the pinned RISC Zero version really uses.
//! It is the only test here that would notice if the digest arithmetic, the
//! byte orders, or the key were subtly wrong, because every one of those
//! produces a valid-looking failure rather than an error.
//!
//! The fixture in `testdata/` took about 22 minutes of CPU to make. Regenerate
//! it from the reproducible guest build:
//!
//! ```text
//! export ZKML_GUEST_DOCKER=1
//! RISC0_DEV_MODE=0 cargo test -p zkml-prover --features groth16 \
//!     --test groth16_bundle real_groth16 -- --ignored
//! cp target/groth16_bundle.json crates/zkml-verifier/testdata/
//! cargo run -p zkml-prover --features zkvm -- export-vk --format json \
//!     -o crates/zkml-verifier/testdata/risc0_vk.json
//! ```
//!
//! These tests read the image id and the RISC Zero parameters from the
//! fixtures, so they cannot tell when either is stale.
//! `crates/zkml-prover/tests/verifier_fixtures.rs` can: on the reproducible
//! build it checks that the receipt's image id is the one this tree builds,
//! and it checks the parameters against the pinned RISC Zero version. See
//! CONTRIBUTING.md, "Changing the zkVM guest".

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Bytes;

fn hex_to_vec(hex: &str) -> std::vec::Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "hex must have an even length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn read(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).expect("valid json")
}

fn bytes(env: &Env, hex: &str) -> Bytes {
    Bytes::from_slice(env, &hex_to_vec(hex))
}

fn bytes32(env: &Env, hex: &str) -> BytesN<32> {
    let array: [u8; 32] = hex_to_vec(hex).try_into().expect("32 bytes");
    BytesN::from_array(env, &array)
}

fn bytes4(env: &Env, hex: &str) -> BytesN<4> {
    let array: [u8; 4] = hex_to_vec(hex).try_into().expect("4 bytes");
    BytesN::from_array(env, &array)
}

/// The fixture's verifying key, optionally with one point replaced.
fn fixture_vk(env: &Env, replace: Option<(&str, std::vec::Vec<u8>)>) -> VerificationKey {
    let vk_json = read("risc0_vk.json");
    let vk = &vk_json["vk"];
    let pick = |name: &str| -> Bytes {
        match &replace {
            Some((target, value)) if *target == name => Bytes::from_slice(env, value),
            _ => bytes(env, vk[name].as_str().unwrap()),
        }
    };
    let mut ic = Vec::new(env);
    for (i, point) in vk["ic"].as_array().unwrap().iter().enumerate() {
        let name = std::format!("ic{i}");
        match &replace {
            Some((target, value)) if *target == name => ic.push_back(Bytes::from_slice(env, value)),
            _ => ic.push_back(bytes(env, point.as_str().unwrap())),
        }
    }
    VerificationKey {
        alpha: pick("alpha"),
        beta: pick("beta"),
        gamma: pick("gamma"),
        delta: pick("delta"),
        ic,
    }
}

/// The contract, initialised for the model the fixture's journal names and
/// configured for the guest that produced it.
fn setup(env: &Env) -> (ZkmlVerifierContractClient<'_>, Bytes, Bytes) {
    env.mock_all_auths();
    let bundle = read("groth16_bundle.json");
    let vk_json = read("risc0_vk.json");

    let seal = bytes(env, bundle["seal"].as_str().unwrap());
    let journal = bytes(env, bundle["journal"].as_str().unwrap());

    // The model hash the journal carries. Registering anything else must
    // make verification fail, which a later test checks.
    let journal_raw = hex_to_vec(bundle["journal"].as_str().unwrap());
    let decoded = zkml_common::journal::JournalV1::decode(&journal_raw).expect("journal");
    let model_hash = Bytes::from_slice(env, &decoded.model_hash);

    let admin = Address::generate(env);

    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), None::<VerificationKey>),
    );
    let client = ZkmlVerifierContractClient::new(env, &contract_id);

    // The image id comes from the bundle, which is self-describing. Any
    // change to the guest or to zkml-common changes the guest's image id,
    // so the fixture has to be regenerated after one; see the module docs.
    client.set_risc0_config(&Risc0Config {
        image_id: bytes32(env, bundle["image_id"].as_str().unwrap()),
        control_root: bytes32(env, vk_json["control_root"].as_str().unwrap()),
        bn254_control_id: bytes32(env, vk_json["bn254_control_id"].as_str().unwrap()),
        selector: bytes4(env, vk_json["selector"].as_str().unwrap()),
    });
    client.set_risc0_vk(&fixture_vk(env, None));

    (client, seal, journal)
}

/// The same setup for other test modules, such as the property tests.
pub(crate) fn setup_pub(env: &Env) -> (ZkmlVerifierContractClient<'_>, Bytes, Bytes) {
    setup(env)
}

#[test]
fn a_real_receipt_verifies_on_chain() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    let record = client.verify_receipt(&seal, &journal);

    // It returns the result it recorded, and that is the one the journal
    // carries.
    assert_eq!(Some(record.clone()), client.get_result());
    assert_eq!(record.class_label, 1);
    assert_eq!(client.get_verification_count(), 1);
}

/// What a real receipt costs on chain.
///
/// This is the number that decides whether the design is usable at all:
/// Soroban caps a transaction at 100 million CPU instructions, and the
/// pairing check alone was already 29 million before receipts existed.
/// Verifying a receipt adds the claim digest, which is eight SHA-256 host
/// calls, and one more scalar multiplication than the native-circuit path.
///
/// Measured on the golden fixture, so the number is the cost of a
/// verification that actually succeeds, not of a cheap early rejection.
///
/// This runs the contract as native Rust, which meters only host calls, so
/// it undercounts: it reports about 30.7M where the compiled WASM costs
/// 33.3M. It stays as a fast regression guard; the number to quote comes
/// from `test_wasm_budget`.
#[test]
fn a_real_receipt_fits_in_the_transaction_budget() {
    use std::println;

    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    env.cost_estimate().budget().reset_default();
    client.verify_receipt(&seal, &journal);

    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let mem = env.cost_estimate().budget().memory_bytes_cost();

    println!("\n=== verify_receipt on a real bundle ===");
    println!("CPU instructions : {}", cpu);
    println!("Memory bytes     : {}", mem);
    println!("=======================================\n");

    // The network limit, which is what actually matters. If this ever
    // fails, receipts cannot be verified on chain at all and the design
    // needs rethinking, not a looser threshold.
    const SOROBAN_CPU_LIMIT: u64 = 100_000_000;
    assert!(
        cpu < SOROBAN_CPU_LIMIT,
        "verify_receipt costs {cpu} instructions and the network allows {SOROBAN_CPU_LIMIT}"
    );

    // A regression guard, generous on purpose: it exists to catch a change
    // that doubles the cost, not to police small movements.
    const MAX_CPU: u64 = 60_000_000;
    const MAX_MEM: u64 = 12_000_000;
    assert!(cpu <= MAX_CPU, "CPU cost regressed: {cpu} over {MAX_CPU}");
    assert!(
        mem <= MAX_MEM,
        "memory cost regressed: {mem} over {MAX_MEM}"
    );
}

#[test]
fn the_same_receipt_cannot_be_used_twice() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    client.verify_receipt(&seal, &journal);
    assert_eq!(
        client.try_verify_receipt(&seal, &journal),
        Err(Ok(VerificationError::ProofAlreadyUsed))
    );
}

#[test]
fn one_flipped_byte_of_the_journal_is_rejected() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    let mut raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
    journal.copy_into_slice(&mut raw);
    raw[72] ^= 0x01; // the output field
    let tampered = Bytes::from_slice(&env, &raw);

    // The journal reaches the proof only through the claim digest, so this
    // has to fail the pairing rather than any cheaper check.
    assert_eq!(
        client.try_verify_receipt(&seal, &tampered),
        Err(Ok(VerificationError::VerificationFailed))
    );
}

#[test]
fn one_flipped_byte_of_the_seal_is_rejected() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    let mut raw = std::vec![0u8; seal.len() as usize];
    seal.copy_into_slice(&mut raw);
    raw[100] ^= 0x01; // inside the B point
    let tampered = Bytes::from_slice(&env, &raw);

    // A typed error, not a host trap: the points are checked before the
    // host sees them.
    assert_eq!(
        client.try_verify_receipt(&tampered, &journal),
        Err(Ok(VerificationError::MalformedProofB))
    );
}

#[test]
fn each_proof_point_reports_its_own_error() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);
    let mut raw = std::vec![0u8; seal.len() as usize];
    seal.copy_into_slice(&mut raw);

    for (pos, expected) in [
        (40usize, VerificationError::MalformedProofA),
        (100, VerificationError::MalformedProofB),
        (220, VerificationError::MalformedProofC),
    ] {
        let mut bad = raw.clone();
        bad[pos] ^= 0x01;
        assert_eq!(
            client.try_verify_receipt(&Bytes::from_slice(&env, &bad), &journal),
            Err(Ok(expected)),
            "byte {pos}"
        );
    }
}

/// Each point of a real seal replaced by the point at infinity, all zero
/// bytes, which the curve checks accept. Refused since VERSION 11.
#[test]
fn a_proof_point_at_infinity_is_refused() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);
    let mut raw = std::vec![0u8; seal.len() as usize];
    seal.copy_into_slice(&mut raw);

    for (range, expected) in [
        (4usize..68, VerificationError::MalformedProofA),
        (68..196, VerificationError::MalformedProofB),
        (196..260, VerificationError::MalformedProofC),
    ] {
        let mut bad = raw.clone();
        bad[range.clone()].fill(0);
        assert_eq!(
            client.try_verify_receipt(&Bytes::from_slice(&env, &bad), &journal),
            Err(Ok(expected)),
            "bytes {range:?}"
        );
    }
    assert_eq!(client.get_verification_count(), 0);
}

#[test]
fn a_coordinate_above_the_field_modulus_is_a_typed_error_not_a_trap() {
    // The host's own on-curve check traps on this, so it is the case that
    // proves the range check runs first.
    let env = Env::default();
    let (client, seal, journal) = setup(&env);
    let mut raw = std::vec![0u8; seal.len() as usize];
    seal.copy_into_slice(&mut raw);

    for (pos, expected) in [
        (4usize, VerificationError::MalformedProofA),
        (68, VerificationError::MalformedProofB),
        (196, VerificationError::MalformedProofC),
    ] {
        let mut bad = raw.clone();
        bad[pos] = 0xff; // coordinate far above p, flag bits set
        assert_eq!(
            client.try_verify_receipt(&Bytes::from_slice(&env, &bad), &journal),
            Err(Ok(expected)),
            "byte {pos}"
        );
    }
}

/// The acceptance criterion, taken literally: any single-byte change to
/// the seal or the journal is rejected with a typed error. Every byte is
/// tried, and not one may abort.
#[test]
fn every_single_byte_change_is_a_typed_error() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    let mut seal_raw = std::vec![0u8; seal.len() as usize];
    seal.copy_into_slice(&mut seal_raw);
    for pos in 0..seal_raw.len() {
        let mut bad = seal_raw.clone();
        bad[pos] ^= 0x01;
        let result = client.try_verify_receipt(&Bytes::from_slice(&env, &bad), &journal);
        assert!(
            matches!(result, Err(Ok(_))),
            "seal byte {pos}: expected a typed error, got {result:?}"
        );
    }

    let mut journal_raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
    journal.copy_into_slice(&mut journal_raw);
    for pos in 0..journal_raw.len() {
        let mut bad = journal_raw;
        bad[pos] ^= 0x01;
        let result = client.try_verify_receipt(&seal, &Bytes::from_slice(&env, &bad));
        assert!(
            matches!(result, Err(Ok(_))),
            "journal byte {pos}: expected a typed error, got {result:?}"
        );
    }

    // And none of those attempts was recorded.
    assert_eq!(client.get_verification_count(), 0);
    // The untouched receipt still verifies afterwards.
    client.verify_receipt(&seal, &journal);
}

#[test]
#[should_panic(expected = "off the curve or at infinity")]
fn a_corrupted_verifying_key_is_refused_at_registration() {
    let env = Env::default();
    let (client, _seal, _journal) = setup(&env);
    let vk_json = read("risc0_vk.json");
    let mut beta = hex_to_vec(vk_json["vk"]["beta"].as_str().unwrap());
    beta[50] ^= 0x01;
    client.set_risc0_vk(&fixture_vk(&env, Some(("beta", beta))));
}

#[test]
fn a_seal_from_another_risc0_version_is_rejected_by_its_selector() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    let mut raw = std::vec![0u8; seal.len() as usize];
    seal.copy_into_slice(&mut raw);
    raw[0] ^= 0xff;
    let tampered = Bytes::from_slice(&env, &raw);

    // Caught before the pairing, and with an error that names the cause.
    assert_eq!(
        client.try_verify_receipt(&tampered, &journal),
        Err(Ok(VerificationError::UnknownSelector))
    );
}

#[test]
fn a_truncated_seal_is_rejected() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);
    let short = seal.slice(0..259);
    assert_eq!(
        client.try_verify_receipt(&short, &journal),
        Err(Ok(VerificationError::MalformedSeal))
    );
}

#[test]
fn a_journal_of_the_wrong_length_is_rejected() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);
    let short = journal.slice(0..95);
    assert_eq!(
        client.try_verify_receipt(&seal, &short),
        Err(Ok(VerificationError::MalformedJournal))
    );
}

#[test]
fn a_receipt_for_another_model_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, seal, journal) = setup(&env);

    // Re-register the contract for a different model.
    client.set_model_hash(&Bytes::from_slice(&env, &[0x11; 32]));
    assert_eq!(
        client.try_verify_receipt(&seal, &journal),
        Err(Ok(VerificationError::VerificationFailed))
    );
}

#[test]
fn a_paused_contract_refuses_receipts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, seal, journal) = setup(&env);

    client.set_pause(&true);
    assert_eq!(
        client.try_verify_receipt(&seal, &journal),
        Err(Ok(VerificationError::ContractPaused))
    );

    // And accepts them again once unpaused, so the pause is a switch and
    // not a one-way door.
    client.set_pause(&false);
    client.verify_receipt(&seal, &journal);
}

#[test]
fn a_journal_with_the_wrong_magic_is_rejected() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    let mut raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
    journal.copy_into_slice(&mut raw);
    raw[0] = b'X'; // ZKML -> XKML
    assert_eq!(
        client.try_verify_receipt(&seal, &Bytes::from_slice(&env, &raw)),
        Err(Ok(VerificationError::MalformedJournal))
    );
}

#[test]
fn a_journal_from_a_future_version_is_rejected() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    // A contract must refuse a layout it was not built for rather than
    // read fields from the positions it happens to expect.
    let mut raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
    journal.copy_into_slice(&mut raw);
    raw[4] = 2; // journal version
    assert_eq!(
        client.try_verify_receipt(&seal, &Bytes::from_slice(&env, &raw)),
        Err(Ok(VerificationError::MalformedJournal))
    );
}

#[test]
fn a_journal_with_a_non_zero_reserved_byte_is_rejected() {
    let env = Env::default();
    let (client, seal, journal) = setup(&env);

    // The guest writes zeros there. Anything else is a different encoding
    // of the same result, and is refused before the pairing.
    for pos in [7usize, 88, 95] {
        let mut raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
        journal.copy_into_slice(&mut raw);
        raw[pos] = 1;
        assert_eq!(
            client.try_verify_receipt(&seal, &Bytes::from_slice(&env, &raw)),
            Err(Ok(VerificationError::MalformedJournal)),
            "reserved byte {pos}"
        );
    }
}

#[test]
fn a_receipt_for_another_guest_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, seal, journal) = setup(&env);

    // Same proof, same journal, but the contract is told a different guest
    // produced it. The image id is inside the claim digest, so this has to
    // fail the pairing.
    let mut cfg = client.get_risc0_config().unwrap();
    let mut id = cfg.image_id.to_array();
    id[0] ^= 0x01;
    cfg.image_id = BytesN::from_array(&env, &id);
    client.set_risc0_config(&cfg);

    assert_eq!(
        client.try_verify_receipt(&seal, &journal),
        Err(Ok(VerificationError::VerificationFailed))
    );
}

#[test]
fn a_wrong_control_root_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, seal, journal) = setup(&env);

    let mut cfg = client.get_risc0_config().unwrap();
    let mut root = cfg.control_root.to_array();
    root[31] ^= 0x01;
    cfg.control_root = BytesN::from_array(&env, &root);
    client.set_risc0_config(&cfg);

    assert_eq!(
        client.try_verify_receipt(&seal, &journal),
        Err(Ok(VerificationError::VerificationFailed))
    );
}

#[test]
#[should_panic(expected = "a RISC Zero verifying key has six ic points")]
fn a_verifying_key_with_five_ic_points_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _seal, _journal) = setup(&env);

    // The key for the native-circuit path has five. Registering it here
    // would fail every receipt, so it is refused at the door.
    let wrong = crate::test_utils::create_dummy_vk(&env, 5);
    client.set_risc0_vk(&wrong);
}

#[test]
fn the_nullifier_depends_on_the_inference() {
    // Only one real proof exists, so this checks the storage directly: the
    // nullifier spent by the real receipt is recorded, and the one a
    // different inference would spend is not. A nullifier that ignored its
    // input would fail the second assertion.
    let env = Env::default();
    let (client, seal, journal) = setup(&env);
    client.verify_receipt(&seal, &journal);

    let mut raw = [0u8; zkml_common::journal::JOURNAL_V1_LEN];
    journal.copy_into_slice(&mut raw);
    let spent = zkml_common::journal::JournalV1::decode(&raw).unwrap();
    raw[80] ^= 0x01; // a different class label
    let other = zkml_common::journal::JournalV1::decode(&raw).unwrap();

    let key_for = |journal: &zkml_common::journal::JournalV1| {
        let inputs = Bytes::from_slice(&env, &journal.public_inputs_bytes());
        (
            NULLIFIER_PREFIX,
            ZkmlVerifierContract::derive_nullifier(&env, &inputs),
        )
    };
    let (spent_key, other_key) = (key_for(&spent), key_for(&other));
    assert_ne!(spent_key, other_key);

    env.as_contract(&client.address, || {
        assert!(env.storage().persistent().has(&spent_key));
        assert!(!env.storage().persistent().has(&other_key));
    });
}

#[test]
fn a_replayed_receipt_is_refused_before_the_pairing() {
    // The replay check depends only on the journal, so a used receipt must
    // be turned away without paying for the pairing. Measured natively,
    // where the pairing host call alone is tens of millions.
    let env = Env::default();
    let (client, seal, journal) = setup(&env);
    client.verify_receipt(&seal, &journal);

    env.cost_estimate().budget().reset_default();
    assert_eq!(
        client.try_verify_receipt(&seal, &journal),
        Err(Ok(VerificationError::ProofAlreadyUsed))
    );
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    assert!(
        cpu < 5_000_000,
        "refusing a replay cost {cpu} instructions; it should not reach the pairing"
    );
}

#[test]
#[should_panic(expected = "off the curve or at infinity")]
fn a_verifying_key_with_gamma_at_infinity_is_refused() {
    // Regression for a forgery: with gamma at infinity, e(L, gamma) = 1 and
    // the journal no longer affects the pairing, so a seal built from the
    // key's own points verifies against any journal.
    let env = Env::default();
    let (client, _seal, _journal) = setup(&env);
    client.set_risc0_vk(&fixture_vk(&env, Some(("gamma", std::vec![0u8; 128]))));
}

#[test]
#[should_panic(expected = "off the curve or at infinity")]
fn a_verifying_key_with_an_ic_point_at_infinity_is_refused() {
    // An ic point at infinity unbinds its public input, here one half of
    // the claim digest.
    let env = Env::default();
    let (client, _seal, _journal) = setup(&env);
    client.set_risc0_vk(&fixture_vk(&env, Some(("ic3", std::vec![0u8; 64]))));
}

#[test]
fn verifying_before_configuring_says_so() {
    let env = Env::default();
    env.mock_all_auths();
    let bundle = read("groth16_bundle.json");
    let seal = bytes(&env, bundle["seal"].as_str().unwrap());
    let journal = bytes(&env, bundle["journal"].as_str().unwrap());

    let admin = Address::generate(&env);
    let vk = crate::test_utils::create_dummy_vk(&env, 5);
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            admin.clone(),
            Bytes::from_slice(&env, &[0x11; 32]),
            Some(vk.clone()),
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    assert_eq!(
        client.try_verify_receipt(&seal, &journal),
        Err(Ok(VerificationError::Risc0NotConfigured))
    );
}
