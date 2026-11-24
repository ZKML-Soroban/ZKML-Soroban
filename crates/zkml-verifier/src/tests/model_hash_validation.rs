//! The model hash a contract is registered with.
//!
//! Every proof's model hash is a Poseidon commitment: 32 bytes, read
//! little-endian, below the BN254 scalar modulus `r`. `parse_public_inputs`
//! already refuses anything else in a proof, so a registered hash outside
//! that range could never be matched and would switch verification off
//! without a word. Since VERSION 11 the constructor and `set_model_hash`
//! refuse it.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;

/// `r` little-endian, the smallest value that is not a valid scalar.
fn r_le() -> [u8; 32] {
    let mut le = BN254_SCALAR_MODULUS;
    le.reverse();
    le
}

/// `r - 1` little-endian, the largest valid scalar.
fn r_minus_one_le() -> [u8; 32] {
    let mut le = r_le();
    le[0] -= 1; // r ends in 0x01, so no borrow
    le
}

fn deploy(env: &Env, model_hash: Bytes) -> ZkmlVerifierContractClient<'_> {
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (Address::generate(env), model_hash, None::<VerificationKey>),
    );
    ZkmlVerifierContractClient::new(env, &contract_id)
}

#[test]
#[should_panic(expected = "model_hash must be 32 bytes below the BN254 scalar modulus")]
fn the_constructor_refuses_a_short_model_hash() {
    let env = Env::default();
    deploy(&env, Bytes::from_slice(&env, &[3u8; 31]));
}

#[test]
#[should_panic(expected = "model_hash must be 32 bytes below the BN254 scalar modulus")]
fn the_constructor_refuses_a_model_hash_equal_to_r() {
    let env = Env::default();
    deploy(&env, Bytes::from_slice(&env, &r_le()));
}

#[test]
fn the_largest_valid_scalar_is_accepted() {
    let env = Env::default();
    let client = deploy(&env, Bytes::from_slice(&env, &r_minus_one_le()));
    assert_eq!(
        client.get_model_hash(),
        Some(Bytes::from_slice(&env, &r_minus_one_le()))
    );
}

#[test]
fn set_model_hash_applies_the_same_rule() {
    let env = Env::default();
    let original = Bytes::from_slice(&env, &[3u8; 32]);
    let client = deploy(&env, original.clone());

    for bad in [
        Bytes::from_slice(&env, &[3u8; 33]),
        Bytes::new(&env),
        Bytes::from_slice(&env, &r_le()),
        Bytes::from_slice(&env, &[0xff; 32]),
    ] {
        assert!(client.try_set_model_hash(&bad).is_err());
        assert_eq!(client.get_model_hash(), Some(original.clone()));
    }
    let good = Bytes::from_slice(&env, &r_minus_one_le());
    client.set_model_hash(&good);
    assert_eq!(client.get_model_hash(), Some(good));
}
