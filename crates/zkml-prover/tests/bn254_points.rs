//! `zkml_common::bn254` against `ark-bn254`.
//!
//! The contract uses `zkml_common::bn254` to reject corrupted proof points with
//! a typed error before the host traps on them. The failure that matters is
//! the opposite one: if that check ever rejected a *valid* point, every honest
//! proof containing it would fail. So this compares it with ark on many valid
//! points, and on corrupted ones, and requires the two to agree exactly.
//!
//! No randomness crate: the scalars come from a fixed sequence, so a failure
//! reproduces.

use ark_bn254::{Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ff::{BigInteger, PrimeField};

use zkml_common::bn254::{g1_is_on_curve, g2_is_on_curve};

/// How many multiples of each generator to test.
const POINTS: u64 = 200;

fn fq(decimal: &str) -> Fq {
    decimal.parse().expect("a valid field element")
}

fn g1_generator() -> G1Affine {
    G1Affine::new_unchecked(fq("1"), fq("2"))
}

/// Read from `ark_bn254::g2` rather than written from memory.
fn g2_generator() -> G2Affine {
    let x = Fq2::new(
        fq("10857046999023057135944570762232829481370756359578518086990519993285655852781"),
        fq("11559732032986387107991004021392285783925812861821192530917403151452391805634"),
    );
    let y = Fq2::new(
        fq("8495653923123431417604973247489272438418190587263600148770280649306958101930"),
        fq("4082367875863433681332203403145435568316851327593401208105741076214120093531"),
    );
    G2Affine::new_unchecked(x, y)
}

/// A spread of scalars from a fixed sequence, small and large.
fn scalar(i: u64) -> Fr {
    let k = i
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(i * i)
        .max(1);
    Fr::from(k)
}

fn be(value: &Fq) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = value.into_bigint().to_bytes_be();
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}

fn encode_g1(p: &G1Affine) -> [u8; 64] {
    if p.infinity {
        return [0u8; 64];
    }
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&be(&p.x));
    out[32..].copy_from_slice(&be(&p.y));
    out
}

fn encode_g2(p: &G2Affine) -> [u8; 128] {
    if p.infinity {
        return [0u8; 128];
    }
    let mut out = [0u8; 128];
    out[0..32].copy_from_slice(&be(&p.x.c1));
    out[32..64].copy_from_slice(&be(&p.x.c0));
    out[64..96].copy_from_slice(&be(&p.y.c1));
    out[96..128].copy_from_slice(&be(&p.y.c0));
    out
}

/// ark's reading of 32 bytes, or `None` where ark would refuse them.
fn ark_fq(bytes: &[u8]) -> Option<Fq> {
    let value = Fq::from_be_bytes_mod_order(bytes);
    // `from_be_bytes_mod_order` reduces silently; reject anything that changed.
    if be(&value)[..] == *bytes {
        Some(value)
    } else {
        None
    }
}

/// What ark says about a G1 encoding: on the curve, or not.
fn ark_g1_on_curve(bytes: &[u8; 64]) -> bool {
    if bytes.iter().all(|b| *b == 0) {
        return true;
    }
    match (ark_fq(&bytes[..32]), ark_fq(&bytes[32..])) {
        (Some(x), Some(y)) => G1Affine::new_unchecked(x, y).is_on_curve(),
        _ => false,
    }
}

/// What ark says about a G2 encoding, read in Soroban's coordinate order.
fn ark_g2_on_curve(bytes: &[u8; 128]) -> bool {
    if bytes.iter().all(|b| *b == 0) {
        return true;
    }
    let parts = [
        ark_fq(&bytes[0..32]),
        ark_fq(&bytes[32..64]),
        ark_fq(&bytes[64..96]),
        ark_fq(&bytes[96..128]),
    ];
    match parts {
        [Some(x_c1), Some(x_c0), Some(y_c1), Some(y_c0)] => {
            G2Affine::new_unchecked(Fq2::new(x_c0, x_c1), Fq2::new(y_c0, y_c1)).is_on_curve()
        }
        _ => false,
    }
}

#[test]
fn every_valid_g1_point_is_accepted() {
    let generator = G1Projective::from(g1_generator());
    for i in 1..=POINTS {
        let point = G1Affine::from(generator * scalar(i));
        let bytes = encode_g1(&point);
        assert!(
            g1_is_on_curve(&bytes),
            "a valid G1 point was rejected (multiple {i})"
        );
    }
}

#[test]
fn every_valid_g2_point_is_accepted() {
    // The case that would break honest proofs, so it gets the most points.
    let generator = G2Projective::from(g2_generator());
    for i in 1..=POINTS {
        let point = G2Affine::from(generator * scalar(i));
        let bytes = encode_g2(&point);
        assert!(
            g2_is_on_curve(&bytes),
            "a valid G2 point was rejected (multiple {i})"
        );
    }
}

#[test]
fn corrupted_g1_points_get_the_same_verdict_as_ark() {
    let generator = G1Projective::from(g1_generator());
    let mut checked = 0;
    for i in 1..=POINTS / 4 {
        let valid = encode_g1(&G1Affine::from(generator * scalar(i)));
        for pos in [0usize, 1, 15, 31, 32, 33, 47, 63] {
            for mask in [0x01u8, 0x80, 0xff] {
                let mut bad = valid;
                bad[pos] ^= mask;
                assert_eq!(
                    g1_is_on_curve(&bad),
                    ark_g1_on_curve(&bad),
                    "disagreement on multiple {i}, byte {pos}, mask {mask:#04x}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "only {checked} corruptions checked");
}

#[test]
fn corrupted_g2_points_get_the_same_verdict_as_ark() {
    let generator = G2Projective::from(g2_generator());
    let mut checked = 0;
    for i in 1..=POINTS / 4 {
        let valid = encode_g2(&G2Affine::from(generator * scalar(i)));
        // One position in and around each of the four coordinates.
        for pos in [0usize, 1, 31, 32, 33, 63, 64, 65, 95, 96, 97, 127] {
            for mask in [0x01u8, 0x80, 0xff] {
                let mut bad = valid;
                bad[pos] ^= mask;
                assert_eq!(
                    g2_is_on_curve(&bad),
                    ark_g2_on_curve(&bad),
                    "disagreement on multiple {i}, byte {pos}, mask {mask:#04x}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 1500, "only {checked} corruptions checked");
}

#[test]
fn the_points_in_the_real_proof_and_key_are_accepted() {
    // Beyond generator multiples: the points a real RISC Zero run produced and
    // the verifying key it checks against.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../zkml-verifier/testdata");
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
    };
    let hex = |s: &str| -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    };

    let seal = hex(read("groth16_bundle.json")["seal"].as_str().unwrap());
    let a: [u8; 64] = seal[4..68].try_into().unwrap();
    let b: [u8; 128] = seal[68..196].try_into().unwrap();
    let c: [u8; 64] = seal[196..260].try_into().unwrap();
    assert!(g1_is_on_curve(&a), "proof point A");
    assert!(g2_is_on_curve(&b), "proof point B");
    assert!(g1_is_on_curve(&c), "proof point C");

    let vk = read("risc0_vk.json");
    let vk = &vk["vk"];
    let alpha: [u8; 64] = hex(vk["alpha"].as_str().unwrap()).try_into().unwrap();
    assert!(g1_is_on_curve(&alpha), "vk alpha");
    for name in ["beta", "gamma", "delta"] {
        let point: [u8; 128] = hex(vk[name].as_str().unwrap()).try_into().unwrap();
        assert!(g2_is_on_curve(&point), "vk {name}");
    }
    for (i, point) in vk["ic"].as_array().unwrap().iter().enumerate() {
        let point: [u8; 64] = hex(point.as_str().unwrap()).try_into().unwrap();
        assert!(g1_is_on_curve(&point), "vk ic[{i}]");
    }
}
