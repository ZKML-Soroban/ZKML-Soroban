extern crate std;
use super::*;
use soroban_sdk::crypto::bn254::{Bn254Fr, Bn254G1Affine};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec as sdk_vec, BytesN, Env};
use std::println;

/// BN254 G1 generator (1, 2) – valid non-infinity point.
const G1_GEN: [u8; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
];

/// Valid non-infinity BN254 G2 point, sourced from the soroban-env-host
/// test suite (g1_pairing_hardcoded).  Serialized in Soroban/Ethereum
/// format: x_imaginary(32) || x_real(32) || y_imaginary(32) || y_real(32),
/// each component big-endian.
pub(crate) const G2_GEN: [u8; 128] = [
    0x1f, 0xb1, 0x9b, 0xb4, 0x76, 0xf6, 0xb9, 0xe4, 0x4e, 0x2a, 0x32, 0x23, 0x4d, 0xa8, 0x21, 0x2f,
    0x61, 0xcd, 0x63, 0x91, 0x93, 0x54, 0xbc, 0x06, 0xae, 0xf3, 0x1e, 0x3c, 0xfa, 0xff, 0x3e, 0xbc,
    0x22, 0x60, 0x68, 0x45, 0xff, 0x18, 0x67, 0x93, 0x91, 0x4e, 0x03, 0xe2, 0x1d, 0xf5, 0x44, 0xc3,
    0x4f, 0xfe, 0x2f, 0x2f, 0x35, 0x04, 0xde, 0x8a, 0x79, 0xd9, 0x15, 0x9e, 0xca, 0x2d, 0x98, 0xd9,
    0x2b, 0xd3, 0x68, 0xe2, 0x83, 0x81, 0xe8, 0xec, 0xcb, 0x5f, 0xa8, 0x1f, 0xc2, 0x6c, 0xf3, 0xf0,
    0x48, 0xee, 0xa9, 0xab, 0xfd, 0xd8, 0x5d, 0x7e, 0xd3, 0xab, 0x36, 0x98, 0xd6, 0x3e, 0x4f, 0x90,
    0x2f, 0xe0, 0x2e, 0x47, 0x88, 0x75, 0x07, 0xad, 0xf0, 0xff, 0x17, 0x43, 0xcb, 0xac, 0x6b, 0xa2,
    0x91, 0xe6, 0x6f, 0x59, 0xbe, 0x6b, 0xd7, 0x63, 0x95, 0x0b, 0xb1, 0x60, 0x41, 0xa0, 0xa8, 0x5e,
];

/// Build a scalar Bn254Fr from a small u64 value (big-endian 32-byte encoding).
fn fr_from_u64(env: &Env, val: u64) -> Bn254Fr {
    let mut buf = [0u8; 32];
    let be = val.to_be_bytes();
    buf[24..32].copy_from_slice(&be);
    Bn254Fr::from_bytes(BytesN::from_array(env, &buf))
}

/// Construct a valid accept-path verification key with non-degenerate points.
///
/// All G2 points (beta, gamma, delta) are a known-good BN254 G2 point
/// from the soroban-env-host test suite.  All IC entries are the BN254
/// G1 generator.
///
/// 5 IC points: IC[0] + model·IC[1] + input·IC[2] + output·IC[3] + class_label·IC[4]
pub fn create_accept_fixture_vk(env: &Env) -> VerificationKey {
    VerificationKey {
        alpha: Bytes::from_slice(env, &G1_GEN),
        beta: Bytes::from_slice(env, &G2_GEN),
        gamma: Bytes::from_slice(env, &G2_GEN),
        delta: Bytes::from_slice(env, &G2_GEN),
        ic: sdk_vec![
            env,
            Bytes::from_slice(env, &G1_GEN),
            Bytes::from_slice(env, &G1_GEN),
            Bytes::from_slice(env, &G1_GEN),
            Bytes::from_slice(env, &G1_GEN),
            Bytes::from_slice(env, &G1_GEN),
        ],
    }
}

/// Compute a proof that satisfies the Groth16 pairing check.
///
/// Given the VK and public-input scalars (model, input, output, class_label),
/// this uses BN254 host functions to compute:
///   L  = IC[0] + model·IC[1] + input·IC[2] + output·IC[3] + class_label·IC[4]
///   C  = −L
///
/// Then the pairing:
///   e(−A, B) · e(α, β) · e(L, γ) · e(C, δ)
/// = e(−G1 + G1 + L − L, G2)
/// = e(O, G2) = 1  ✓
pub fn compute_valid_proof(
    env: &Env,
    vk: &VerificationKey,
    model_scalar: u64,
    input_scalar: u64,
    output_scalar: u64,
    class_label_scalar: u64,
) -> (Bytes, Bytes, Bytes) {
    let bn254 = env.crypto().bn254();

    // Deserialize IC[0] as the starting point for L
    let ic0_bytes = vk.ic.get(0).unwrap();
    let mut ic0_arr = [0u8; 64];
    ic0_bytes.copy_into_slice(&mut ic0_arr);
    let mut l = Bn254G1Affine::from_array(env, &ic0_arr);

    // L += model_scalar * IC[1]
    let ic1_bytes = vk.ic.get(1).unwrap();
    let mut ic1_arr = [0u8; 64];
    ic1_bytes.copy_into_slice(&mut ic1_arr);
    let ic1 = Bn254G1Affine::from_array(env, &ic1_arr);
    let term1 = bn254.g1_mul(&ic1, &fr_from_u64(env, model_scalar));
    l = bn254.g1_add(&l, &term1);

    // L += input_scalar * IC[2]
    let ic2_bytes = vk.ic.get(2).unwrap();
    let mut ic2_arr = [0u8; 64];
    ic2_bytes.copy_into_slice(&mut ic2_arr);
    let ic2 = Bn254G1Affine::from_array(env, &ic2_arr);
    let term2 = bn254.g1_mul(&ic2, &fr_from_u64(env, input_scalar));
    l = bn254.g1_add(&l, &term2);

    // L += output_scalar * IC[3]
    let ic3_bytes = vk.ic.get(3).unwrap();
    let mut ic3_arr = [0u8; 64];
    ic3_bytes.copy_into_slice(&mut ic3_arr);
    let ic3 = Bn254G1Affine::from_array(env, &ic3_arr);
    let term3 = bn254.g1_mul(&ic3, &fr_from_u64(env, output_scalar));
    l = bn254.g1_add(&l, &term3);

    // L += class_label_scalar * IC[4]
    let ic4_bytes = vk.ic.get(4).unwrap();
    let mut ic4_arr = [0u8; 64];
    ic4_bytes.copy_into_slice(&mut ic4_arr);
    let ic4 = Bn254G1Affine::from_array(env, &ic4_arr);
    let term4 = bn254.g1_mul(&ic4, &fr_from_u64(env, class_label_scalar));
    l = bn254.g1_add(&l, &term4);

    // C = −L  (proof_c)
    let c = -l;

    // A = G1 generator, B = G2 point
    let proof_a = Bytes::from_slice(env, &G1_GEN);
    let proof_b = Bytes::from_slice(env, &G2_GEN);
    let proof_c = Bytes::from_slice(env, &c.to_array());

    (proof_a, proof_b, proof_c)
}

/// Like [`compute_valid_proof`], for any 80-byte public inputs, including
/// full 32-byte commitments. Each field is read little-endian, as
/// `verify_inference` reads it, but converted here on its own rather than
/// through the contract's helpers, so a conversion bug there cannot cancel out.
pub fn compute_valid_proof_for(
    env: &Env,
    vk: &VerificationKey,
    public_inputs: &Bytes,
) -> (Bytes, Bytes, Bytes) {
    let bn254 = env.crypto().bn254();
    let ic = |i: u32| {
        let mut arr = [0u8; 64];
        vk.ic.get(i).unwrap().copy_into_slice(&mut arr);
        Bn254G1Affine::from_array(env, &arr)
    };
    let mut l = ic(0);
    for (k, (start, end)) in [(0u32, 32u32), (32, 64), (64, 72), (72, 80)]
        .into_iter()
        .enumerate()
    {
        let mut be = [0u8; 32];
        for (j, byte) in public_inputs.slice(start..end).iter().enumerate() {
            be[31 - j] = byte;
        }
        let scalar = Bn254Fr::from_bytes(BytesN::from_array(env, &be));
        l = bn254.g1_add(&l, &bn254.g1_mul(&ic(k as u32 + 1), &scalar));
    }
    let c = -l;
    (
        Bytes::from_slice(env, &G1_GEN),
        Bytes::from_slice(env, &G2_GEN),
        Bytes::from_slice(env, &c.to_array()),
    )
}

#[test]
fn test_verifier_accept_path_and_resource_budget() {
    let env = Env::default();

    // --- Public-input scalars: model=3, input=5, output=42, class_label=7. ---
    // bytes_to_fr interprets input as little-endian, so we encode
    // each scalar as [val, 0, 0, ..., 0] (32 bytes LE for model/input,
    // 8 bytes LE for output/class_label i64).
    let model_le = {
        let mut b = [0u8; 32];
        b[0] = 3;
        b
    };
    let input_le = {
        let mut b = [0u8; 32];
        b[0] = 5;
        b
    };
    // Output is a single i64 scalar in LE (8 bytes)
    let output_le = {
        let mut b = [0u8; 8];
        b[0] = 42;
        b
    };
    // Class label is a single i64 scalar in LE (8 bytes)
    let class_label_le = {
        let mut b = [0u8; 8];
        b[0] = 7;
        b
    };

    // --- VK with non-degenerate G2 and non-zero IC ---
    let model_hash = Bytes::from_slice(&env, &model_le);
    let vk = create_accept_fixture_vk(&env);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (admin.clone(), model_hash.clone(), Some(vk.clone())),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    // --- Proof that genuinely satisfies the pairing ---
    let (proof_a, proof_b, proof_c) = compute_valid_proof(&env, &vk, 3, 5, 42, 7);

    // Public inputs: model(32) + input(32) + output(8) + class_label(8) = 80 bytes
    // Parsed as: 1 model + 1 input + 1 output + 1 class_label = 4 scalars → IC[0..4]
    let mut public_inputs_vec = [0u8; 80];
    public_inputs_vec[0..32].copy_from_slice(&model_le);
    public_inputs_vec[32..64].copy_from_slice(&input_le);
    public_inputs_vec[64..72].copy_from_slice(&output_le);
    public_inputs_vec[72..80].copy_from_slice(&class_label_le);
    let public_inputs = Bytes::from_slice(&env, &public_inputs_vec);

    // --- Measure resource budget ---
    env.cost_estimate().budget().reset_default();

    let result = client.try_verify_inference(&proof_a, &proof_b, &proof_c, &public_inputs);
    assert_eq!(result, Ok(Ok(())));

    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let mem = env.cost_estimate().budget().memory_bytes_cost();

    println!("\n=== Verifier Resource Budget Harness Report ===");
    println!("CPU Instructions : {}", cpu);
    println!("Memory Bytes     : {}", mem);
    println!("===============================================\n");

    // Regression threshold assertion: fail if cost regresses sharply.
    // Thresholds are generous to avoid flaky tests; the point is to
    // catch large regressions, not to be tight.
    const MAX_CPU_THRESHOLD: u64 = 50_000_000;
    const MAX_MEM_THRESHOLD: u64 = 10_000_000;

    assert!(
        cpu <= MAX_CPU_THRESHOLD,
        "Verifier CPU instruction cost regressed sharply: got {}, threshold {}",
        cpu,
        MAX_CPU_THRESHOLD
    );
    assert!(
        mem <= MAX_MEM_THRESHOLD,
        "Verifier Memory bytes cost regressed sharply: got {}, threshold {}",
        mem,
        MAX_MEM_THRESHOLD
    );
}
