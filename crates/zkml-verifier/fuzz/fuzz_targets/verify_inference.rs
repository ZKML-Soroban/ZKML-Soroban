//! `verify_inference` must never trap: every input verifies or returns a
//! `VerificationError`. Half the runs start from a proof that verifies and
//! overwrite a few bytes, so the fuzzer reaches the point checks and the
//! pairing rather than stopping at the length checks.
//!
//! The one documented exception is a crafted proof B on the curve but outside
//! the G2 subgroup, which the host's pairing traps on; a random G2 encoding is
//! on the curve with probability about 1/p.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::crypto::bn254::{Bn254Fr, Bn254G1Affine};
use soroban_sdk::testutils::{Address as _, EnvTestConfig};
use soroban_sdk::{vec, Address, Bytes, BytesN, Env};
use zkml_verifier::{VerificationKey, ZkmlVerifierContract, ZkmlVerifierContractClient};

/// BN254 G1 generator (1, 2).
const G1_GEN: [u8; 64] = {
    let mut p = [0u8; 64];
    p[31] = 1;
    p[63] = 2;
    p
};

/// A valid G2 point from the soroban-env-host test suite, the same one the
/// contract's own accept-path tests use.
const G2_GEN: [u8; 128] = [
    0x1f, 0xb1, 0x9b, 0xb4, 0x76, 0xf6, 0xb9, 0xe4, 0x4e, 0x2a, 0x32, 0x23, 0x4d, 0xa8, 0x21, 0x2f,
    0x61, 0xcd, 0x63, 0x91, 0x93, 0x54, 0xbc, 0x06, 0xae, 0xf3, 0x1e, 0x3c, 0xfa, 0xff, 0x3e, 0xbc,
    0x22, 0x60, 0x68, 0x45, 0xff, 0x18, 0x67, 0x93, 0x91, 0x4e, 0x03, 0xe2, 0x1d, 0xf5, 0x44, 0xc3,
    0x4f, 0xfe, 0x2f, 0x2f, 0x35, 0x04, 0xde, 0x8a, 0x79, 0xd9, 0x15, 0x9e, 0xca, 0x2d, 0x98, 0xd9,
    0x2b, 0xd3, 0x68, 0xe2, 0x83, 0x81, 0xe8, 0xec, 0xcb, 0x5f, 0xa8, 0x1f, 0xc2, 0x6c, 0xf3, 0xf0,
    0x48, 0xee, 0xa9, 0xab, 0xfd, 0xd8, 0x5d, 0x7e, 0xd3, 0xab, 0x36, 0x98, 0xd6, 0x3e, 0x4f, 0x90,
    0x2f, 0xe0, 0x2e, 0x47, 0x88, 0x75, 0x07, 0xad, 0xf0, 0xff, 0x17, 0x43, 0xcb, 0xac, 0x6b, 0xa2,
    0x91, 0xe6, 0x6f, 0x59, 0xbe, 0x6b, 0xd7, 0x63, 0x95, 0x0b, 0xb1, 0x60, 0x41, 0xa0, 0xa8, 0x5e,
];

#[derive(Arbitrary, Debug)]
enum Input {
    /// Proof points and public inputs of any length and content.
    Raw {
        a: Vec<u8>,
        b: Vec<u8>,
        c: Vec<u8>,
        public_inputs: Vec<u8>,
    },
    /// A valid proof with these (position, byte) overwrites.
    Corrupted { edits: Vec<(u16, u8)> },
}

fn fr(env: &Env, value: u64) -> Bn254Fr {
    let mut be = [0u8; 32];
    be[24..].copy_from_slice(&value.to_be_bytes());
    Bn254Fr::from_bytes(BytesN::from_array(env, &be))
}

fuzz_target!(|input: Input| {
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    let g1 = Bytes::from_slice(&env, &G1_GEN);
    let g2 = Bytes::from_slice(&env, &G2_GEN);
    let vk = VerificationKey {
        alpha: g1.clone(),
        beta: g2.clone(),
        gamma: g2.clone(),
        delta: g2.clone(),
        ic: vec![&env, g1.clone(), g1.clone(), g1.clone(), g1.clone(), g1.clone()],
    };
    // Public inputs: model 3, input 5, output 42, class label 7, little-endian.
    let mut inputs = [0u8; 80];
    inputs[0] = 3;
    inputs[32] = 5;
    inputs[64] = 42;
    inputs[72] = 7;
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(&env),
            Bytes::from_slice(&env, &inputs[..32]),
            Some(vk),
        ),
    );
    let client = ZkmlVerifierContractClient::new(&env, &contract_id);

    let (a, b, c, pi) = match input {
        Input::Raw {
            a,
            b,
            c,
            public_inputs,
        } => (a, b, c, public_inputs),
        Input::Corrupted { edits } => {
            // Every ic point is G1, so L = (1 + 3 + 5 + 42 + 7) * G1, and with
            // A = G1, B = beta and C = -L the pairing product is 1.
            let bn254 = env.crypto().bn254();
            let gen = Bn254G1Affine::from_array(&env, &G1_GEN);
            let l = bn254.g1_mul(&gen, &fr(&env, 58));
            let mut buf = Vec::with_capacity(336);
            buf.extend_from_slice(&G1_GEN);
            buf.extend_from_slice(&G2_GEN);
            buf.extend_from_slice(&(-l).to_array());
            buf.extend_from_slice(&inputs);
            for (pos, byte) in edits {
                let pos = pos as usize % buf.len();
                buf[pos] = byte;
            }
            (
                buf[0..64].to_vec(),
                buf[64..192].to_vec(),
                buf[192..256].to_vec(),
                buf[256..].to_vec(),
            )
        }
    };
    let bytes = |v: &[u8]| Bytes::from_slice(&env, v);
    let result = client.try_verify_inference(&bytes(&a), &bytes(&b), &bytes(&c), &bytes(&pi));
    assert!(
        matches!(result, Ok(_) | Err(Ok(_))),
        "verify_inference trapped: {result:?}"
    );
});
