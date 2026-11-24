//! G2 points on the curve but outside the prime-order subgroup.
//!
//! BN254's G2 has a large cofactor, so the curve equation alone admits points
//! no honest key or proof contains. `zkml_common::bn254::g2_is_on_curve` checks
//! only the equation, and Soroban has no host function for subgroup membership,
//! but its pairing traps on such a point. Up to VERSION 10 a key holding one
//! was stored, and then every verification trapped; since VERSION 11 the
//! registration itself fails, through a pairing on the key's G2 points.

use super::*;
use ark_bn254::{Fq, Fq2, G2Affine};
use ark_ec::AffineRepr;
use ark_ff::{BigInteger, PrimeField};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;
use test_utils::create_dummy_vk;

/// The Soroban encoding of a G2 point: be(x.c1) || be(x.c0) || be(y.c1) || be(y.c0).
fn encode(point: &G2Affine) -> [u8; 128] {
    let (x, y) = point.xy().expect("not the point at infinity");
    let mut out = [0u8; 128];
    for (i, coordinate) in [x.c1, x.c0, y.c1, y.c0].iter().enumerate() {
        out[32 * i..32 * (i + 1)].copy_from_slice(&coordinate.into_bigint().to_bytes_be());
    }
    out
}

/// The first point with x = (k, 1) that is on the curve and outside the
/// subgroup. Built with arkworks, independently of the contract's checks.
fn off_subgroup_g2() -> [u8; 128] {
    (1u64..)
        .find_map(|k| {
            let x = Fq2::new(Fq::from(k), Fq::from(1u64));
            G2Affine::get_point_from_x_unchecked(x, false)
                .filter(|p| p.is_on_curve() && !p.is_in_correct_subgroup_assuming_on_curve())
        })
        .map(|p| encode(&p))
        .unwrap()
}

fn deploy(env: &Env, vk: Option<VerificationKey>) -> ZkmlVerifierContractClient<'_> {
    env.mock_all_auths();
    let contract_id = env.register(
        ZkmlVerifierContract,
        (
            Address::generate(env),
            Bytes::from_slice(env, &[3u8; 32]),
            vk,
        ),
    );
    ZkmlVerifierContractClient::new(env, &contract_id)
}

#[test]
fn the_curve_equation_alone_accepts_the_point() {
    // So `key_points_are_valid`, which checks the equation, cannot keep it
    // out of a key on its own.
    assert!(zkml_common::bn254::g2_is_on_curve(&off_subgroup_g2()));
}

#[test]
fn set_risc0_vk_refuses_a_beta_outside_the_subgroup() {
    let env = Env::default();
    let client = deploy(&env, None);
    let mut vk = create_dummy_vk(&env, 6);
    vk.beta = Bytes::from_slice(&env, &off_subgroup_g2());

    assert!(client.try_set_risc0_vk(&vk).is_err());
    assert!(
        client.get_risc0_vk().is_none(),
        "the key must not be stored"
    );
    // The same key with a valid beta is accepted.
    client.set_risc0_vk(&create_dummy_vk(&env, 6));
}

#[test]
fn set_verification_key_refuses_a_delta_outside_the_subgroup() {
    let env = Env::default();
    let client = deploy(&env, None);
    let mut vk = create_dummy_vk(&env, 5);
    vk.delta = Bytes::from_slice(&env, &off_subgroup_g2());

    assert!(client.try_set_verification_key(&vk).is_err());
    let (a, b, c) = test_utils::dummy_proof(&env);
    assert_eq!(
        client.try_verify_inference(&a, &b, &c, &Bytes::from_slice(&env, &[0u8; 80])),
        Err(Ok(VerificationError::NativeKeyNotConfigured))
    );
}

#[test]
#[should_panic(expected = "point not in the correct subgroup")]
fn the_constructor_refuses_a_gamma_outside_the_subgroup() {
    let env = Env::default();
    let mut vk = create_dummy_vk(&env, 5);
    vk.gamma = Bytes::from_slice(&env, &off_subgroup_g2());
    deploy(&env, Some(vk));
}
