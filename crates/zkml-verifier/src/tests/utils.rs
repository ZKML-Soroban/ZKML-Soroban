use super::*;
use soroban_sdk::Env;

/// Create a dummy verification key for testing.
/// Uses the G1 generator point (1, 2) which is valid.
/// num_ic specifies the number of IC points (must be >= 3 for model_hash, input_hash, output).
pub fn create_dummy_vk(env: &Env, num_ic: u32) -> VerificationKey {
    let g1_bytes: [u8; 64] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 2,
    ];
    // Valid points, so the constructor accepts the key, but with a known
    // trapdoor: fine for tests, never for a deployment.
    let g2_bytes = crate::test_budget::G2_GEN;
    let _g1 = Bn254G1Affine::from_array(env, &g1_bytes);
    let _g2 = Bn254G2Affine::from_array(env, &g2_bytes);

    let mut ic = Vec::new(env);
    for _ in 0..num_ic {
        ic.push_back(Bytes::from_slice(env, &g1_bytes));
    }

    VerificationKey {
        alpha: Bytes::from_slice(env, &g1_bytes),
        beta: Bytes::from_slice(env, &g2_bytes),
        gamma: Bytes::from_slice(env, &g2_bytes),
        delta: Bytes::from_slice(env, &g2_bytes),
        ic,
    }
}

/// Create dummy proof components for testing.
pub fn dummy_proof(env: &Env) -> (Bytes, Bytes, Bytes) {
    let proof_a = Bytes::from_slice(env, &[0u8; 64]);
    let proof_b = Bytes::from_slice(env, &[0u8; 128]);
    let proof_c = Bytes::from_slice(env, &[0u8; 64]);
    (proof_a, proof_b, proof_c)
}
