# Groth16 Fixtures

This directory contains Groth16 verification key, proof, and public input fixtures for testing the zkml-verifier contract.

## Regeneration Steps

To regenerate these fixtures:

1. **Navigate to the prover crate:**
   ```bash
   cd crates/zkml-prover
   ```

2. **Run the fixture generation example:**
   ```bash
   cargo run --example generate_groth16_fixtures
   ```

This will:
- Generate a minimal Groth16 circuit with 3 public inputs (model_hash, input_hash, output)
- Create a verification key and proof using arkworks
- Serialize the verification key, proof, and public inputs to JSON format
- Write the fixtures to this directory

## Fixture Files

- `verification_key.json`: Groth16 verification key with alpha, beta, gamma, delta, and IC elements
- `proof.json`: Groth16 proof with A, B, and C points
- `public_inputs.json`: Public inputs including model_hash, input_hash, and output

## Circuit Structure

The minimal circuit has:
- 3 public inputs: model_hash, input_hash, output
- A simple constraint: model_hash + input_hash = output
- This ensures the circuit is satisfiable and generates a valid proof

## Serialization Format

- Field elements are serialized as 32-byte little-endian values
- G1 points are serialized as 64-byte Ethereum-compatible format (x || y)
- G2 points are serialized as 128-byte Ethereum-compatible format (x || y)

## Notes

The fixtures are generated using a deterministic RNG seed (42) for reproducibility. The circuit is designed to exercise the verifier's pairing arithmetic and scalar conversion logic without requiring complex ML inference logic.
