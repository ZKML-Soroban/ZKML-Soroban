//! Proof-related data structures.
//!
//! These types represent the data exchanged between the off-chain prover
//! and the on-chain verifier contract.

use serde::{Deserialize, Serialize};

/// Opaque byte wrapper for a Groth16 proof serialized according to the
/// BN254 curve encoding expected by the Soroban host functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Groth16Proof {
    /// Serialized proof bytes (A, B, C curve points).
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
    /// `model_hash (32) || input_hash (32) || output`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.output.len());
        out.extend_from_slice(&self.model_hash);
        out.extend_from_slice(&self.input_hash);
        out.extend_from_slice(&self.output);
        out
    }
}
