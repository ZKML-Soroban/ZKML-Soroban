//! `verify_receipt` must never trap: every seal and journal verifies or
//! returns a `VerificationError`. Half the runs start from the real receipt in
//! `testdata/` and overwrite a few bytes, so the fuzzer gets past the selector
//! and the journal layout to the point checks and the pairing.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::testutils::{Address as _, EnvTestConfig};
use soroban_sdk::{Address, Bytes, BytesN, Env, Vec as SdkVec};
use zkml_verifier::{Risc0Config, VerificationKey, ZkmlVerifierContract, ZkmlVerifierContractClient};

const BUNDLE: &str = include_str!("../../testdata/groth16_bundle.json");
const RISC0_VK: &str = include_str!("../../testdata/risc0_vk.json");

#[derive(Arbitrary, Debug)]
enum Input {
    Raw { seal: Vec<u8>, journal: Vec<u8> },
    Corrupted { edits: Vec<(u16, u8)> },
}

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fuzz_target!(|input: Input| {
    let bundle: serde_json::Value = serde_json::from_str(BUNDLE).unwrap();
    let params: serde_json::Value = serde_json::from_str(RISC0_VK).unwrap();
    let field = |v: &serde_json::Value, k: &str| unhex(v[k].as_str().unwrap());

    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    let journal = field(&bundle, "journal");
    // JournalV1: magic (4), version (2), kind (2), then the model hash.
    let model_hash = Bytes::from_slice(&env, &journal[8..40]);
    let contract_id = env.register(
        ZkmlVerifierContract,
        (Address::generate(&env), model_hash, None::<VerificationKey>),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let b32 = |v: Vec<u8>| BytesN::<32>::from_array(&env, &v.try_into().unwrap());
    client.set_risc0_config(&Risc0Config {
        image_id: b32(field(&bundle, "image_id")),
        control_root: b32(field(&params, "control_root")),
        bn254_control_id: b32(field(&params, "bn254_control_id")),
        selector: BytesN::from_array(&env, &field(&params, "selector").try_into().unwrap()),
    });
    let vk = &params["vk"];
    let mut ic = SdkVec::new(&env);
    for point in vk["ic"].as_array().unwrap() {
        ic.push_back(Bytes::from_slice(&env, &unhex(point.as_str().unwrap())));
    }
    client.set_risc0_vk(&VerificationKey {
        alpha: Bytes::from_slice(&env, &field(vk, "alpha")),
        beta: Bytes::from_slice(&env, &field(vk, "beta")),
        gamma: Bytes::from_slice(&env, &field(vk, "gamma")),
        delta: Bytes::from_slice(&env, &field(vk, "delta")),
        ic,
    });

    let (seal, journal) = match input {
        Input::Raw { seal, journal } => (seal, journal),
        Input::Corrupted { edits } => {
            let mut buf = field(&bundle, "seal");
            buf.extend_from_slice(&journal);
            for (pos, byte) in edits {
                let pos = pos as usize % buf.len();
                buf[pos] = byte;
            }
            let journal = buf.split_off(260);
            (buf, journal)
        }
    };
    let result = client.try_verify_receipt(
        &Bytes::from_slice(&env, &seal),
        &Bytes::from_slice(&env, &journal),
    );
    assert!(
        matches!(result, Ok(_) | Err(Ok(_))),
        "verify_receipt trapped: {result:?}"
    );
});
