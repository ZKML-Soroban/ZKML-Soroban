//! Generate valid Groth16 verification key and proof fixtures for testing.
//!
//! This creates a minimal circuit that matches the public-input structure
//! expected by the verifier contract: model_hash (32 bytes), input_hash (32 bytes),
//! and output (variable bytes).
//!
//! ```text
//! cargo run -p zkml-prover --example generate_groth16_fixtures
//! ```

use ark_bn254::{Bn254, Fr as Bn254Fr};
use ark_groth16::Groth16;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use ark_std::rand::rngs::StdRng;
use ark_std::rand::SeedableRng;
use std::fs;
use std::path::PathBuf;

/// A minimal circuit that exercises the public-input structure.
///
/// This circuit has 3 public inputs to match the verifier's L computation:
/// - input[0]: model_hash (as field element)
/// - input[1]: input_hash (as field element)  
/// - input[2]: output (as field element)
///
/// The verifier computes: L = IC[0] + model_hash*IC[1] + input_hash*IC[2] + output*IC[3]
/// where IC[0] is the constant term from the verification key.
///
/// The circuit has a simple constraint: model_hash + input_hash = output
/// This ensures the circuit is satisfiable and generates a valid proof.
#[derive(Clone)]
struct MinimalCircuit {
    model_hash: Option<Bn254Fr>,
    input_hash: Option<Bn254Fr>,
    output: Option<Bn254Fr>,
}

impl ConstraintSynthesizer<Bn254Fr> for MinimalCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Bn254Fr>) -> Result<(), SynthesisError> {
        // Allocate public inputs
        let model_hash_var =
            cs.new_input_variable(|| self.model_hash.ok_or(SynthesisError::AssignmentMissing))?;
        let input_hash_var =
            cs.new_input_variable(|| self.input_hash.ok_or(SynthesisError::AssignmentMissing))?;
        let output_var =
            cs.new_input_variable(|| self.output.ok_or(SynthesisError::AssignmentMissing))?;

        // Add a simple constraint: model_hash + input_hash = output
        cs.enforce_constraint(
            ark_relations::r1cs::LinearCombination::zero() + model_hash_var + input_hash_var,
            ark_relations::r1cs::LinearCombination::from(ark_relations::r1cs::Variable::One),
            ark_relations::r1cs::LinearCombination::zero() + output_var,
        )?;

        Ok(())
    }
}

/// Serialize a field element to little-endian bytes (matching Soroban's bytes_to_fr)
/// The verifier expects 32 bytes in little-endian format
fn fr_to_le_bytes(fr: &Bn254Fr) -> Vec<u8> {
    let mut bytes = Vec::new();
    fr.serialize_uncompressed(&mut bytes).unwrap();
    // arkworks serializes in big-endian, convert to little-endian
    bytes.reverse();
    // Truncate or pad to 32 bytes (the verifier expects exactly 32 bytes)
    if bytes.len() > 32 {
        bytes.truncate(32);
    } else {
        while bytes.len() < 32 {
            bytes.push(0);
        }
    }
    bytes
}

/// Serialize G1 point to Ethereum-compatible 64-byte format
fn g1_to_bytes(g1: &ark_bn254::G1Affine) -> Vec<u8> {
    let mut bytes = Vec::new();
    g1.serialize_uncompressed(&mut bytes).unwrap();
    // arkworks serializes as: flag (1 byte) || x (32 bytes) || y (32 bytes)
    // We need to remove the flag byte for Ethereum format
    if bytes.len() == 65 {
        bytes.remove(0);
    }
    bytes
}

/// Serialize G2 point to Ethereum-compatible 128-byte format
fn g2_to_bytes(g2: &ark_bn254::G2Affine) -> Vec<u8> {
    let mut bytes = Vec::new();
    g2.serialize_uncompressed(&mut bytes).unwrap();
    // arkworks serializes as: flag (1 byte) || x (64 bytes) || y (64 bytes)
    // We need to remove the flag byte for Ethereum format
    if bytes.len() == 129 {
        bytes.remove(0);
    }
    bytes
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("groth16");
    fs::create_dir_all(&out_dir)?;

    println!("Generating Groth16 fixtures...");

    // Use deterministic RNG for reproducible fixtures
    let rng = &mut StdRng::seed_from_u64(42);

    // Generate sample public inputs (non-trivial scalar values)
    // Ensure the constraint model_hash + input_hash = output is satisfied
    let model_hash = Bn254Fr::from(12345u64);
    let input_hash = Bn254Fr::from(67890u64);
    let output = model_hash + input_hash; // Enforced by circuit

    let circuit = MinimalCircuit {
        model_hash: Some(model_hash),
        input_hash: Some(input_hash),
        output: Some(output),
    };

    println!("Generating setup (this may take a moment)...");
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit.clone(), rng)?;

    println!("Generating proof...");
    let proof = Groth16::<Bn254>::prove(&pk, circuit, rng)?;

    // Serialize verification key
    println!("Serializing verification key...");
    let vk_data = VerificationKeyData {
        alpha: g1_to_bytes(&vk.alpha_g1),
        beta: g2_to_bytes(&vk.beta_g2),
        gamma: g2_to_bytes(&vk.gamma_g2),
        delta: g2_to_bytes(&vk.delta_g2),
        ic: vk.gamma_abc_g1.iter().map(g1_to_bytes).collect(),
    };

    let vk_json = serde_json::to_string_pretty(&vk_data)?;
    fs::write(out_dir.join("verification_key.json"), &vk_json)?;

    // Serialize proof
    println!("Serializing proof...");
    let proof_data = ProofData {
        a: g1_to_bytes(&proof.a),
        b: g2_to_bytes(&proof.b),
        c: g1_to_bytes(&proof.c),
    };

    let proof_json = serde_json::to_string_pretty(&proof_data)?;
    fs::write(out_dir.join("proof.json"), &proof_json)?;

    // Serialize public inputs
    println!("Serializing public inputs...");
    // Public inputs order for the verifier: dummy (0) || model_hash || input_hash || output
    // The verifier expects: model_hash (32 bytes) || input_hash (32 bytes) || output (variable)
    // But the circuit has: dummy || model_hash || input_hash || output
    // So we serialize the actual values (model_hash, input_hash, output) for the test
    let public_inputs = PublicInputsData {
        model_hash: fr_to_le_bytes(&model_hash),
        input_hash: fr_to_le_bytes(&input_hash),
        output: fr_to_le_bytes(&output),
    };

    let inputs_json = serde_json::to_string_pretty(&public_inputs)?;
    fs::write(out_dir.join("public_inputs.json"), &inputs_json)?;

    println!("Fixtures written to {}", out_dir.display());
    println!("Files generated:");
    println!("  - verification_key.json");
    println!("  - proof.json");
    println!("  - public_inputs.json");

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct VerificationKeyData {
    alpha: Vec<u8>,
    beta: Vec<u8>,
    gamma: Vec<u8>,
    delta: Vec<u8>,
    ic: Vec<Vec<u8>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ProofData {
    a: Vec<u8>,
    b: Vec<u8>,
    c: Vec<u8>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PublicInputsData {
    model_hash: Vec<u8>,
    input_hash: Vec<u8>,
    output: Vec<u8>,
}
