---
title: "Glossary"
description: "Terms used across the zkml-soroban documentation."
icon: "book"
---

| Term | Meaning |
| ---- | ------- |
| **Anchor** | A regulated Stellar on/off-ramp institution; a primary target user of provable KYC scoring. |
| **BN254** | Pairing-friendly elliptic curve (alt-bn128) used by Groth16; exposed as Soroban host functions by CAP-0074. |
| **class_label** | Public input with the model decision: threshold result (logistic regression), argmax (MLP), `0` (decision tree). |
| **Commitment** | 32-byte Poseidon hash binding a proof to a model or an input. |
| **Fixed-point (Q16.16)** | Integer encoding of reals with 16 fractional bits, stored in `i64`. |
| **Groth16** | zk-SNARK with constant-size proofs (three curve points) and constant-time pairing verification. |
| **IC points** | The verification key's G1 points used to fold public inputs into `L`. |
| **Image ID** | Hash identifying a RISC Zero guest program; receipts are verified against it. |
| **Journal** | Public output committed by a RISC Zero guest. |
| **Nullifier** | `sha256(public_inputs)` stored on-chain to prevent recording the same result twice. |
| **Poseidon** | ZK-friendly algebraic hash; exposed as host functions by CAP-0075. |
| **Prover** | `zkml-prover`, the off-chain crate/CLI that runs model inference and generates ZK proof bundles. |
| **Public inputs** | Values revealed to the verifier with a proof: here the 80-byte `model_hash \|\| input_hash \|\| output \|\| class_label`. |
| **Q16.16** | Shorthand for the fixed-point format used to encode model and input values; see Fixed-point (Q16.16). |
| **Quantization** | Converting floating-point model parameters to fixed-point. |
| **Receipt** | RISC Zero proof of guest execution (STARK), wrappable into Groth16. |
| **Route A / Route B** | Proving via the RISC Zero zkVM (Phase 1) or native model-specific circuits (Phase 2). |
| **Trace** | The zkVM guest's execution record (register and memory state per cycle) that a STARK proof commits to and proves correct. |
| **TTL** | Time to live of Soroban storage entries, extended to keep state live. |
| **Verification key (VK)** | Groth16 parameters (`alpha`, `beta`, `gamma`, `delta`, `ic`) registered in the contract. |
| **Verifier** | `zkml-verifier`, the on-chain Soroban contract that checks a submitted Groth16 proof against a registered Verification key. |
| **zkVM** | Virtual machine that proves correct execution of a program (RISC Zero). |
