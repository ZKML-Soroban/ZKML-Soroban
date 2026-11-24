//! The on-chain cost, measured on the compiled WASM.
//!
//! Every other test here runs the contract as native Rust, and the native test
//! environment only meters host function calls. Code that runs inside the
//! contract, like the G2 on-curve check or the claim digest's buffer building,
//! is free there and not free on chain. This loads the real `.wasm` so the
//! budget counts WASM instructions too.
//!
//! Ignored by default because it needs the artifact built first. CI runs it
//! in the `Verifier WASM` job, right after building the contract:
//!
//! ```text
//! cargo build -p zkml-verifier --target wasm32v1-none --profile contract
//! cargo test -p zkml-verifier wasm_budget -- --ignored --nocapture
//! ```
//!
//! `ZKML_VERIFIER_WASM` points it at a different build, which is how a change
//! is compared against the commit before it.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Bytes;
use std::println;

fn hex_to_vec(hex: &str) -> std::vec::Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

/// The measured figures in `budget-baseline.json`, next to `Cargo.toml`.
///
/// Each test fails when its figure moves above the baseline by more than the
/// file's tolerance, so a change that moves the cost noticeably re-measures it
/// and updates the file and `docs/reference/benchmarks.md` together, instead of
/// the docs drifting away from the contract. The size test also checks the
/// network's limit.
fn baseline(key: &str) -> u64 {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("budget-baseline.json");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    json[key]
        .as_u64()
        .unwrap_or_else(|| panic!("{key} missing from budget-baseline.json"))
}

/// Fail if `measured` is above the baseline for `key` plus the tolerance.
fn check_against_baseline(key: &str, measured: u64) {
    let base = baseline(key);
    let ceiling = base + base * baseline("tolerance_percent") / 100;
    println!("{key}: {measured} (baseline {base}, ceiling {ceiling})");
    assert!(
        measured <= ceiling,
        "{key} is {measured}, above its baseline {base} plus {}%; re-measure and update          budget-baseline.json and docs/reference/benchmarks.md",
        baseline("tolerance_percent")
    );
    assert!(measured < baseline("network_cpu_limit") || key == "wasm_bytes");
}

fn wasm_path() -> std::string::String {
    std::env::var("ZKML_VERIFIER_WASM").unwrap_or_else(|_| {
        std::format!(
            "{}/../../target/wasm32v1-none/contract/zkml_verifier.wasm",
            env!("CARGO_MANIFEST_DIR")
        )
    })
}

fn read_wasm() -> std::vec::Vec<u8> {
    let path = wasm_path();
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}; build the contract first"))
}

/// The WASM must fit the network's `contract_max_size_bytes` and stay near its
/// baseline. Replaces CI's old size report, which printed the size and
/// checked nothing.
#[test]
#[ignore = "needs the WASM built first; see the module docs"]
fn wasm_budget_size() {
    let size = read_wasm().len() as u64;
    let limit = baseline("network_contract_max_size_bytes");
    println!(
        "
=== WASM size: {size} bytes, network limit {limit}"
    );
    assert!(
        size <= limit,
        "{size} bytes does not fit the network limit of {limit}"
    );
    check_against_baseline("wasm_bytes", size);
}

fn read(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
#[ignore = "needs the WASM built first; see the module docs"]
fn wasm_budget() {
    let wasm_path = wasm_path();
    let wasm = read_wasm();

    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    env.cost_estimate().budget().reset_unlimited();

    let bundle = read("groth16_bundle.json");
    let vk_json = read("risc0_vk.json");
    let b = |hex: &str| Bytes::from_slice(&env, &hex_to_vec(hex));

    let journal_raw = hex_to_vec(bundle["journal"].as_str().unwrap());
    let decoded = zkml_common::journal::JournalV1::decode(&journal_raw).unwrap();

    let admin = Address::generate(&env);

    let contract_id = env.register(
        wasm.as_slice(),
        (
            admin,
            Bytes::from_slice(&env, &decoded.model_hash),
            None::<VerificationKey>,
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let b32 = |hex: &str| -> BytesN<32> {
        BytesN::from_array(&env, &hex_to_vec(hex).try_into().expect("32 bytes"))
    };
    let b4 = |hex: &str| -> BytesN<4> {
        BytesN::from_array(&env, &hex_to_vec(hex).try_into().expect("4 bytes"))
    };
    client.set_risc0_config(&Risc0Config {
        image_id: b32(bundle["image_id"].as_str().unwrap()),
        control_root: b32(vk_json["control_root"].as_str().unwrap()),
        bn254_control_id: b32(vk_json["bn254_control_id"].as_str().unwrap()),
        selector: b4(vk_json["selector"].as_str().unwrap()),
    });
    let vk = &vk_json["vk"];
    let mut ic = Vec::new(&env);
    for point in vk["ic"].as_array().unwrap() {
        ic.push_back(b(point.as_str().unwrap()));
    }
    // Registering the key runs a pairing on its G2 points (the subgroup
    // check), so it is measured too: it has to fit in one transaction.
    env.cost_estimate().budget().reset_unlimited();
    client.set_risc0_vk(&VerificationKey {
        alpha: b(vk["alpha"].as_str().unwrap()),
        beta: b(vk["beta"].as_str().unwrap()),
        gamma: b(vk["gamma"].as_str().unwrap()),
        delta: b(vk["delta"].as_str().unwrap()),
        ic,
    });
    let register_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    println!("\n=== set_risc0_vk CPU instructions: {register_cpu}");
    check_against_baseline("set_risc0_vk_cpu", register_cpu);

    let seal = b(bundle["seal"].as_str().unwrap());
    let journal = b(bundle["journal"].as_str().unwrap());

    env.cost_estimate().budget().reset_unlimited();
    client.verify_receipt(&seal, &journal);
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let mem = env.cost_estimate().budget().memory_bytes_cost();

    println!("\n=== verify_receipt, WASM: {wasm_path}");
    println!("WASM size        : {} bytes", wasm.len());
    println!("CPU instructions : {cpu}");
    println!("Memory bytes     : {mem}");

    check_against_baseline("verify_receipt_cpu", cpu);
}

/// `verify_inference` on the same WASM, so the two entry points can be
/// compared like for like. Uses the accept-path fixture from `test_budget`,
/// a proof that genuinely satisfies the pairing equation.
#[test]
#[ignore = "needs the WASM built first; see the module docs"]
fn wasm_budget_verify_inference() {
    let wasm_path = wasm_path();
    let wasm = read_wasm();

    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    env.cost_estimate().budget().reset_unlimited();
    // Same scalars and layout as test_budget: model=3, input=5, output=42,
    // class_label=7, each little-endian in the 80-byte layout.
    let mut inputs = [0u8; 80];
    inputs[0] = 3;
    inputs[32] = 5;
    inputs[64] = 42;
    inputs[72] = 7;
    let model_hash = Bytes::from_slice(&env, &inputs[0..32]);
    let vk = crate::test_budget::create_accept_fixture_vk(&env);
    let contract_id = env.register(
        wasm.as_slice(),
        (Address::generate(&env), model_hash, Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);
    let (a, b, c) = crate::test_budget::compute_valid_proof(&env, &vk, 3, 5, 42, 7);
    let public_inputs = Bytes::from_slice(&env, &inputs);

    env.cost_estimate().budget().reset_unlimited();
    client.verify_inference(&a, &b, &c, &public_inputs);
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let mem = env.cost_estimate().budget().memory_bytes_cost();

    println!("\n=== verify_inference, WASM: {wasm_path}");
    println!("CPU instructions : {cpu}");
    println!("Memory bytes     : {mem}");

    check_against_baseline("verify_inference_cpu", cpu);
}
