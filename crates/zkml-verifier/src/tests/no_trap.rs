//! Property tests: no input makes a verification entry point trap.
//!
//! A trap aborts the transaction in the host without saying why, so every
//! input either verifies or comes back as a `VerificationError`. Through a
//! `try_` call a trap is `Err(Err(_))`; a typed error is `Err(Ok(_))`.
//!
//! Two kinds of input: arbitrary bytes of arbitrary lengths, which mostly stop
//! at the cheap checks, and a valid proof with a few bytes overwritten, which
//! reaches the point checks and the pairing. The one documented exception, a
//! crafted proof B on the curve but outside the G2 subgroup, does not come out
//! of either: a random G2 encoding is on the curve with probability about 1/p.
//! The weekly `cargo fuzz` job runs the same properties far longer.
//!
//! The cases come from a fixed-seed xorshift generator rather than `proptest`,
//! which would pull in a `getrandom` that does not build on the GNU Windows
//! toolchain. A failure prints the case number and the input.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::EnvTestConfig;
use soroban_sdk::Env;

/// An environment that writes no test snapshot when dropped: these tests
/// create hundreds, and a snapshot of each would only be noise in the repo.
fn quiet_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn inputs() -> [u8; 80] {
    let mut inputs = [0u8; 80];
    inputs[0] = 3;
    inputs[32] = 5;
    inputs[64] = 42;
    inputs[72] = 7;
    inputs
}

/// The valid proof and public inputs, as one 336-byte buffer:
/// A (64) || B (128) || C (64) || public inputs (80).
fn valid_native(env: &Env) -> (ZkmlVerifierContractClient<'_>, std::vec::Vec<u8>) {
    let vk = crate::test_budget::create_accept_fixture_vk(env);
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(env),
            Bytes::from_slice(env, &inputs()[..32]),
            Some(vk.clone()),
        ),
    );
    let (a, b, c) = crate::test_budget::compute_valid_proof(env, &vk, 3, 5, 42, 7);
    let mut buf = std::vec::Vec::new();
    for part in [a, b, c] {
        let mut raw = std::vec![0u8; part.len() as usize];
        part.copy_into_slice(&mut raw);
        buf.extend_from_slice(&raw);
    }
    buf.extend_from_slice(&inputs());
    (ZkmlVerifierContractClient::new(env, &contract_id), buf)
}

fn typed<T, E>(result: &Result<T, Result<VerificationError, E>>) -> bool {
    matches!(result, Ok(_) | Err(Ok(_)))
}

/// xorshift64*: deterministic, so a failing case reproduces exactly.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn bytes(&mut self, max_len: usize) -> std::vec::Vec<u8> {
        let len = self.below(max_len + 1);
        (0..len).map(|_| self.next() as u8).collect()
    }
    /// Overwrite one to five random positions of `buf`.
    fn corrupt(&mut self, buf: &mut [u8]) {
        for _ in 0..1 + self.below(5) {
            let pos = self.below(buf.len());
            buf[pos] = self.next() as u8;
        }
    }
}

const CASES: u64 = 200;

#[test]
fn verify_inference_never_traps_on_arbitrary_bytes() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for case in 0..CASES {
        let env = quiet_env();
        let (client, _) = valid_native(&env);
        let parts = [rng.bytes(80), rng.bytes(140), rng.bytes(80), rng.bytes(100)];
        let b = |v: &[u8]| Bytes::from_slice(&env, v);
        let result =
            client.try_verify_inference(&b(&parts[0]), &b(&parts[1]), &b(&parts[2]), &b(&parts[3]));
        assert!(
            typed(&result),
            "case {case} trapped on {parts:?}: {result:?}"
        );
    }
}

#[test]
fn verify_inference_never_traps_on_a_corrupted_valid_proof() {
    let mut rng = Rng(0xd1b5_4a32_d192_ed03);
    for case in 0..CASES {
        let env = quiet_env();
        let (client, mut buf) = valid_native(&env);
        rng.corrupt(&mut buf);
        let b = |r: core::ops::Range<usize>| Bytes::from_slice(&env, &buf[r]);
        let result =
            client.try_verify_inference(&b(0..64), &b(64..192), &b(192..256), &b(256..336));
        assert!(typed(&result), "case {case} trapped on {buf:?}: {result:?}");
    }
}

#[test]
fn verify_receipt_never_traps_on_arbitrary_bytes() {
    let mut rng = Rng(0x8cb9_2ba7_2f3d_8dd7);
    for case in 0..CASES {
        let env = quiet_env();
        let (client, _, _) = crate::test_golden_receipt::setup_pub(&env);
        let (seal, journal) = (rng.bytes(300), rng.bytes(120));
        let result = client.try_verify_receipt(
            &Bytes::from_slice(&env, &seal),
            &Bytes::from_slice(&env, &journal),
        );
        assert!(
            typed(&result),
            "case {case} trapped on {seal:?} {journal:?}: {result:?}"
        );
    }
}

#[test]
fn verify_receipt_never_traps_on_a_corrupted_real_receipt() {
    let mut rng = Rng(0x6a09_e667_f3bc_c908);
    for case in 0..CASES {
        let env = quiet_env();
        let (client, seal, journal) = crate::test_golden_receipt::setup_pub(&env);
        let mut buf = std::vec![0u8; 356];
        seal.copy_into_slice(&mut buf[..260]);
        journal.copy_into_slice(&mut buf[260..]);
        rng.corrupt(&mut buf);
        let result = client.try_verify_receipt(
            &Bytes::from_slice(&env, &buf[..260]),
            &Bytes::from_slice(&env, &buf[260..]),
        );
        assert!(typed(&result), "case {case} trapped on {buf:?}: {result:?}");
    }
}
