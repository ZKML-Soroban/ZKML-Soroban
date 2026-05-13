# Proof Generation Pipeline

Phase 1 generates proofs with the RISC Zero zkVM and wraps the receipt into a
Groth16 proof suitable for the BN254 host functions on Soroban.

## Steps

1. Commit to the model parameters (`model_commitment`) and the input features
   (`input_commitment`).
2. Run inference inside the zkVM guest, producing a journal (public outputs)
   and a receipt (proof).
3. Convert the STARK receipt to a Groth16 SNARK (Bonsai or local prover).
4. Package the proof and public inputs into a `VerificationBundle`.

## Public Inputs

| Field        | Meaning                                  |
| ------------ | ---------------------------------------- |
| `model_hash` | Commitment to the model parameters.      |
| `input_hash` | Commitment to the input feature vector.  |
| `output`     | The inference result as field elements.  |
