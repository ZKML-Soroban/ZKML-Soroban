//! RISC Zero's universal Groth16 verifying key, in the contract's byte layout.
//!
//! A Groth16 receipt is verified against a verifying key that RISC Zero fixes
//! for a given version. The contract needs that key as bytes, but
//! `risc0_groth16::VerifyingKey` keeps its points private and exposes them only
//! through serde, which writes ark's uncompressed little-endian encoding.
//!
//! So the key is round-tripped: serialize what risc0 pins, parse it with ark,
//! and re-emit each point in the big-endian layout Soroban's BN254 host
//! functions read (CAP-0074, the same as EIP-196 / EIP-197).
//!
//! Deriving the key from the pinned crate rather than copying constants means a
//! RISC Zero version bump regenerates it. A key that silently disagrees with
//! the proofs is the kind of failure that shows up as "verification failed"
//! with nothing to point at.

use ark_bn254::{Bn254, Fq, Fq2, G1Affine, G2Affine};
use ark_ff::{BigInteger, PrimeField};
use ark_serialize::CanonicalDeserialize;

use crate::prover::ProveError;

/// A G1 point: `be(x) || be(y)`.
pub type G1Bytes = [u8; 64];
/// A G2 point: `be(x.c1) || be(x.c0) || be(y.c1) || be(y.c0)`.
pub type G2Bytes = [u8; 128];

/// RISC Zero's universal verifying key, ready for `initialize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Groth16VerifyingKey {
    /// `alpha` in G1.
    pub alpha: G1Bytes,
    /// `beta` in G2.
    pub beta: G2Bytes,
    /// `gamma` in G2.
    pub gamma: G2Bytes,
    /// `delta` in G2.
    pub delta: G2Bytes,
    /// `ic`, one point per public input plus one. A RISC Zero receipt has five
    /// public inputs, so this holds six points.
    pub ic: Vec<G1Bytes>,
}

/// A field element as 32 big-endian bytes.
fn fq_be(value: &Fq) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = value.into_bigint().to_bytes_be();
    // `to_bytes_be` can be shorter than 32 bytes for small values; right-align.
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}

/// A G1 point as `be(x) || be(y)`.
///
/// The coordinates are read from the affine fields rather than through
/// `AffineRepr::xy`, so this needs no extra dependency on `ark-ec`.
fn g1_bytes(point: &G1Affine) -> Result<G1Bytes, ProveError> {
    if point.infinity {
        return Err(ProveError::Zkvm(
            "verifying key holds the point at infinity in G1".into(),
        ));
    }
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&fq_be(&point.x));
    out[32..].copy_from_slice(&fq_be(&point.y));
    Ok(out)
}

/// An Fq2 element as `be(c1) || be(c0)`, which is the order Soroban reads.
fn fq2_bytes(value: &Fq2) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&fq_be(&value.c1));
    out[32..].copy_from_slice(&fq_be(&value.c0));
    out
}

/// A G2 point as `be(x) || be(y)` with each coordinate in Fq2 order.
fn g2_bytes(point: &G2Affine) -> Result<G2Bytes, ProveError> {
    if point.infinity {
        return Err(ProveError::Zkvm(
            "verifying key holds the point at infinity in G2".into(),
        ));
    }
    let mut out = [0u8; 128];
    out[..64].copy_from_slice(&fq2_bytes(&point.x));
    out[64..].copy_from_slice(&fq2_bytes(&point.y));
    Ok(out)
}

/// Read the verifying key that the pinned RISC Zero version verifies against.
#[cfg(feature = "zkvm")]
pub fn universal_verifying_key() -> Result<Groth16VerifyingKey, ProveError> {
    let params = risc0_zkvm::Groth16ReceiptVerifierParameters::default();

    // `risc0_groth16::VerifyingKey` is a newtype whose field serializes through
    // ark's uncompressed encoding, so serde hands back exactly those bytes.
    let encoded: Vec<u8> =
        serde_json::from_value(serde_json::to_value(&params.verifying_key).map_err(|e| {
            ProveError::Serialization(format!("serializing the verifying key: {e}"))
        })?)
        .map_err(|e| ProveError::Serialization(format!("reading the verifying key bytes: {e}")))?;

    let vk = ark_groth16::VerifyingKey::<Bn254>::deserialize_uncompressed(encoded.as_slice())
        .map_err(|e| ProveError::Zkvm(format!("parsing the verifying key: {e}")))?;

    let ic = vk
        .gamma_abc_g1
        .iter()
        .map(g1_bytes)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Groth16VerifyingKey {
        alpha: g1_bytes(&vk.alpha_g1)?,
        beta: g2_bytes(&vk.beta_g2)?,
        gamma: g2_bytes(&vk.gamma_g2)?,
        delta: g2_bytes(&vk.delta_g2)?,
        ic,
    })
}

#[cfg(all(test, feature = "zkvm"))]
mod tests {
    use super::*;

    #[test]
    fn the_key_has_six_ic_points_for_five_public_inputs() {
        let vk = universal_verifying_key().expect("the pinned version has a verifying key");
        assert_eq!(
            vk.ic.len(),
            6,
            "a RISC Zero receipt has five public inputs, so ic is one longer"
        );
    }

    #[test]
    fn points_are_not_all_zero() {
        // A point of all zeroes is the encoding for infinity, which would make
        // the pairing check pass against anything.
        let vk = universal_verifying_key().unwrap();
        assert_ne!(vk.alpha, [0u8; 64]);
        assert_ne!(vk.beta, [0u8; 128]);
        assert_ne!(vk.gamma, [0u8; 128]);
        assert_ne!(vk.delta, [0u8; 128]);
        for (i, point) in vk.ic.iter().enumerate() {
            assert_ne!(*point, [0u8; 64], "ic[{i}] is the point at infinity");
        }
    }

    #[test]
    fn every_coordinate_is_below_the_field_modulus() {
        // Soroban traps on a coordinate that is not reduced, so catch it here
        // where the error says what is wrong.
        const Q: [u8; 32] = [
            0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81,
            0x58, 0x5d, 0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16,
            0xd8, 0x7c, 0xfd, 0x47,
        ];
        let vk = universal_verifying_key().unwrap();
        let check = |bytes: &[u8], what: &str| {
            for (i, coord) in bytes.chunks(32).enumerate() {
                assert!(coord < &Q[..], "{what} coordinate {i} is not reduced");
            }
        };
        check(&vk.alpha, "alpha");
        check(&vk.beta, "beta");
        check(&vk.gamma, "gamma");
        check(&vk.delta, "delta");
        for (i, point) in vk.ic.iter().enumerate() {
            check(point, &format!("ic[{i}]"));
        }
    }

    #[test]
    fn the_key_is_deterministic() {
        assert_eq!(
            universal_verifying_key().unwrap(),
            universal_verifying_key().unwrap()
        );
    }
}
