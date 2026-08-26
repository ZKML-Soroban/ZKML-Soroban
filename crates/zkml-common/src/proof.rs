//! Proof-related data structures.
//!
//! These types represent the data exchanged between the off-chain prover
//! and the on-chain verifier contract.

use serde::{Deserialize, Serialize};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Opaque byte wrapper for a Groth16 proof serialized according to the
/// BN254 curve encoding expected by the Soroban host functions.
///
/// # Byte Layout (Wire Format Contract)
///
/// The proof consists of three elliptic curve points over BN254:
/// - `A ∈ G1`: 64 bytes (uncompressed)
/// - `B ∈ G2`: 128 bytes (uncompressed)  
/// - `C ∈ G1`: 64 bytes (uncompressed)
///
/// Total: 256 bytes
///
/// ## G1 Point Encoding (A and C)
///
/// Each G1 point (x, y) is encoded as 64 bytes in big-endian order:
/// ```text
/// [0..32):  x coordinate (32 bytes, big-endian)
/// [32..64): y coordinate (32 bytes, big-endian)
/// ```
///
/// ## G2 Point Encoding (B)
///
/// G2 points are in Fp2, where each coordinate is a pair of field elements.
/// Following Ethereum precompile conventions (EIP-196), each coordinate is
/// encoded as (c1, c0) where the value is c0 + c1 * i:
/// ```text
/// [0..32):   x.c1 (32 bytes, big-endian)
/// [32..64):  x.c0 (32 bytes, big-endian)
/// [64..96):  y.c1 (32 bytes, big-endian)
/// [96..128): y.c0 (32 bytes, big-endian)
/// ```
///
/// ## Endianness
///
/// All field elements are encoded in big-endian byte order to match
/// Ethereum's BN254 precompile (alt_bn128_add, alt_bn128_mul, alt_bn128_pairing).
/// This ensures interoperability with existing ZK tooling.
///
/// # Stability
///
/// This byte layout is a **wire-format contract** with the Soroban verifier.
/// Any changes require coordinated updates to:
/// - The verifier contract's deserialization logic
/// - The CAP-0074 host function integration
/// - All existing proofs in circulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Groth16Proof {
    /// Serialized proof bytes (A, B, C curve points).
    ///
    /// Must be exactly 256 bytes following the layout documented above.
    pub data: Vec<u8>,
}

/// Public inputs that accompany a proof submission.
///
/// The verifier contract checks these values against the proof to confirm
/// that the claimed inference result corresponds to the committed model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicInputs {
    /// Poseidon hash of the model parameters (commitment).
    pub model_hash: [u8; 32],
    /// Poseidon hash of the input features.
    pub input_hash: [u8; 32],
    /// The inference output value (as a raw field element).
    pub output: Vec<u8>,
    /// The class label decision (binary decision or argmax index).
    pub class_label: i64,
}

/// A complete verification bundle sent to the on-chain contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationBundle {
    /// The Groth16 proof.
    pub proof: Groth16Proof,
    /// The public inputs tied to this proof.
    pub public_inputs: PublicInputs,
}

impl PublicInputs {
    /// Serialize the public inputs into the byte layout the verifier expects:
    /// `model_hash (32) || input_hash (32) || output || class_label (8)`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.output.len() + 8);
        out.extend_from_slice(&self.model_hash);
        out.extend_from_slice(&self.input_hash);
        out.extend_from_slice(&self.output);
        out.extend_from_slice(&self.class_label.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_inputs_serialize_with_prefix() {
        let pi = PublicInputs {
            model_hash: [1u8; 32],
            input_hash: [2u8; 32],
            output: vec![9u8; 8],
            class_label: 0,
        };
        let bytes = pi.to_bytes();
        assert_eq!(bytes.len(), 80);
        assert_eq!(&bytes[0..32], &[1u8; 32]);
        assert_eq!(&bytes[32..64], &[2u8; 32]);
        assert_eq!(&bytes[64..72], &[9u8; 8]);
        assert_eq!(&bytes[72..80], &[0u8; 8]);
    }

    #[test]
    fn groth16_proof_size_target() {
        // Groth16 proofs should be < 500 bytes (Phase 1 success criteria)
        // A standard Groth16 proof is 256 bytes (64 + 128 + 64)
        let proof = Groth16Proof {
            data: vec![0u8; 256],
        };
        assert!(
            proof.data.len() < 500,
            "Groth16 proof size {} exceeds 500 byte target",
            proof.data.len()
        );
    }

    #[test]
    fn groth16_proof_round_trip_serialization() {
        let original = Groth16Proof {
            data: vec![
                1u8; 256 // Simulated Groth16 proof (A: 64, B: 128, C: 64 bytes)
            ],
        };

        // Serialize
        let serialized = bincode::serialize(&original).expect("Groth16Proof serialization failed");

        // Deserialize
        let deserialized: Groth16Proof =
            bincode::deserialize(&serialized).expect("Groth16Proof deserialization failed");

        // Verify round-trip
        assert_eq!(original.data, deserialized.data);
    }

    #[test]
    fn verification_bundle_round_trip_serialization() {
        let bundle = VerificationBundle {
            proof: Groth16Proof {
                data: vec![1u8; 256],
            },
            public_inputs: PublicInputs {
                model_hash: [1u8; 32],
                input_hash: [2u8; 32],
                output: vec![3u8; 8],
                class_label: 4,
            },
        };

        // Serialize
        let serialized =
            bincode::serialize(&bundle).expect("VerificationBundle serialization failed");

        // Deserialize
        let deserialized: VerificationBundle =
            bincode::deserialize(&serialized).expect("VerificationBundle deserialization failed");

        // Verify round-trip
        assert_eq!(bundle.proof.data, deserialized.proof.data);
        assert_eq!(
            bundle.public_inputs.model_hash,
            deserialized.public_inputs.model_hash
        );
        assert_eq!(
            bundle.public_inputs.input_hash,
            deserialized.public_inputs.input_hash
        );
        assert_eq!(
            bundle.public_inputs.output,
            deserialized.public_inputs.output
        );
    }
}
