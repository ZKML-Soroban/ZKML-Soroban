# Technical Overview

This document provides a detailed technical description of the cryptographic
primitives, protocol features, design decisions, and implementation strategies
that underpin zkml-soroban.

---

## Table of Contents

- [ZK Primitives on Stellar](#zk-primitives-on-stellar)
  - [BN254 Elliptic Curve (CAP-0074)](#bn254-elliptic-curve-cap-0074)
  - [Poseidon Hash Function (CAP-0075)](#poseidon-hash-function-cap-0075)
  - [Groth16 Proof System](#groth16-proof-system)
- [Protocol Context](#protocol-context)
- [Fixed-Point Arithmetic](#fixed-point-arithmetic)
- [Model Quantization Strategy](#model-quantization-strategy)
- [ONNX Compatibility](#onnx-compatibility)
- [Prover Stack](#prover-stack)
  - [Phase 1: RISC Zero zkVM](#phase-1-risc-zero-zkvm)
  - [Phase 2: Native Circuits](#phase-2-native-circuits)
- [Verifier Contract Design](#verifier-contract-design)
- [Benchmarking Considerations](#benchmarking-considerations)

---

## ZK Primitives on Stellar

### BN254 Elliptic Curve (CAP-0074)

BN254 (also known as alt-bn128) is a pairing-friendly elliptic curve widely
used in zero-knowledge proof systems. Protocol 25 (X-Ray) introduced native
Soroban host functions for BN254 operations, including:

- **Point addition** on G1 and G2 groups.
- **Scalar multiplication** on G1 and G2.
- **Pairing check** (bilinear map): the critical operation for Groth16
  verification.

These host functions execute at the Stellar Core level (not in WASM), making
them orders of magnitude faster than implementing curve arithmetic in
contract code. This is what makes on-chain Groth16 verification practical.

The BN254 curve operates over a 254-bit prime field, providing approximately
128 bits of security. This matches the security level of most deployed
ZK-SNARK systems.

### Poseidon Hash Function (CAP-0075)

Poseidon is a family of hash functions designed specifically for efficient
evaluation inside ZK circuits. Unlike SHA-256 or Keccak, Poseidon uses
algebraic operations (exponentiations over finite fields) that map directly
to arithmetic constraints, resulting in significantly fewer gates per hash
invocation.

The Soroban host functions support:

- Configurable permutation widths.
- Operations over both BLS12-381 Fr and BN254 Fr field elements.
- Standard domain separation for different use cases.

In zkml-soroban, Poseidon is used for two critical purposes:

1. **Model commitment**: A Poseidon hash of all quantized model parameters
   serves as the on-chain identifier for a specific model version. This
   binding ensures that a proof generated for model M cannot be used to
   claim results for model M'.

2. **Input commitment**: A Poseidon hash of the input feature vector is
   included in the public inputs. This prevents a prover from substituting
   inputs after proof generation.

### Groth16 Proof System

Groth16 is a zero-knowledge succinct non-interactive argument of knowledge
(zk-SNARK) with the following properties:

- **Constant proof size**: A Groth16 proof consists of three elliptic curve
  points (A in G1, B in G2, C in G1), totaling approximately 128 bytes
  regardless of the computation size.
- **Constant verification time**: Verification requires a fixed number of
  pairing operations (three pairings and one multi-exponentiation), making
  it ideal for on-chain execution.
- **Trusted setup**: Groth16 requires a circuit-specific trusted setup
  ceremony. For Phase 1 (RISC Zero), this is handled by the zkVM
  infrastructure. For Phase 2 (native circuits), a setup ceremony must be
  conducted for each circuit.

The verification equation is:

```
e(A, B) = e(alpha, beta) * e(L, gamma) * e(C, delta)
```

Where:
- `(A, B, C)` are the proof elements.
- `(alpha, beta, gamma, delta)` are the verification key elements.
- `L = sum(public_input_i * vk_ic_i)` is the linear combination of public
  inputs with the verification key's IC array.

---

## Protocol Context

### Protocol 25: X-Ray

Released January 22, 2026, X-Ray introduced the BN254 and Poseidon host
functions that form the cryptographic foundation of this project. Before
X-Ray, building ZK applications on Stellar required implementing curve
arithmetic in WASM, which was prohibitively expensive.

Key capabilities enabled by X-Ray:

- Verification of Groth16 proofs produced by any compatible prover
  (RISC Zero, Circom, snarkjs).
- On-chain model commitments using ZK-friendly hashing.
- Interoperability with Ethereum ZK tooling (BN254 is the same curve used
  by Ethereum's precompiled contracts).

### Protocol 26: Yardstick

Entered testnet April 16, 2026, Yardstick adds benchmarking tools and new
host functions. While not directly required by zkml-soroban, the
benchmarking infrastructure will be valuable for measuring and optimizing
verification gas costs.

---

## Fixed-Point Arithmetic

ZK circuits operate over finite fields, which represent integers modulo a
large prime. Floating-point operations are incompatible with this
constraint system. The solution is fixed-point arithmetic.

### Representation

zkml-soroban uses the Q16.16 format:

- Values are multiplied by `2^16 = 65536` and stored as 64-bit signed
  integers.
- This provides approximately 4-5 decimal digits of fractional precision.
- The integer part uses 47 bits (plus 1 sign bit), supporting values up
  to approximately +/- 140 trillion.

### Operations

| Operation       | Implementation                     | Overflow Risk |
|-----------------|------------------------------------|---------------|
| Addition        | Direct integer addition            | Low           |
| Subtraction     | Direct integer subtraction         | Low           |
| Multiplication  | `(a * b) >> scale`                 | Medium        |
| Comparison      | Direct integer comparison          | None          |

Multiplication requires a right shift to maintain the correct scale. For
intermediate products, 128-bit integers may be needed to avoid overflow
before the shift. This is handled natively in Rust and translates to
range check constraints in ZK circuits.

### Precision Validation

Before running inference, the quantization module validates that:

1. All weight values fall within the representable range.
2. No intermediate computation can overflow i64 bounds during inference
   (computed via static analysis of the model graph).
3. The quantized model's predictions have at most a configurable deviation
   from the original floating-point model (measured on a validation
   dataset).

---

## Model Quantization Strategy

The quantization pipeline converts a floating-point ONNX model into a
fixed-point representation:

1. **Parameter extraction**: Parse the ONNX protobuf to extract weights,
   biases, tree thresholds, and other numeric parameters.

2. **Range analysis**: Compute the minimum and maximum values across all
   parameters to determine if the default scale factor is appropriate.

3. **Uniform quantization**: Multiply all values by `2^16` and round to
   the nearest integer. This is a symmetric, uniform quantization scheme.

4. **Validation**: Run the quantized model on a held-out dataset and
   compare predictions against the original floating-point model.
   Report accuracy metrics and any significant deviations.

5. **Commitment**: Compute a Poseidon hash over the complete set of
   quantized parameters. This hash becomes the on-chain model identifier.

### Supported ONNX Operators

| ONNX Operator             | Target Model            | Notes                        |
|---------------------------|-------------------------|------------------------------|
| `TreeEnsembleClassifier`  | DecisionTree            | Single tree or ensemble      |
| `LinearClassifier`        | LogisticRegression      | Binary or multi-class        |
| `MatMul`                  | TinyMLP (dense layer)   | Requires matching `Add`      |
| `Add`                     | TinyMLP (bias)          | Paired with `MatMul`         |
| `Relu`                    | TinyMLP (activation)    | max(0, x) in fixed-point     |

---

## ONNX Compatibility

The project targets ONNX opset version 17 or later. Models can be exported
from:

- **scikit-learn**: via `skl2onnx` for decision trees and logistic
  regression.
- **PyTorch**: via `torch.onnx.export` for small MLPs.
- **ONNX Runtime**: for validation of the imported model.

The ONNX importer is intentionally limited to the operators listed above.
Unsupported operators will produce a clear error message at import time
rather than silently failing.

---

## Prover Stack

### Phase 1: RISC Zero zkVM

The RISC Zero zkVM executes arbitrary Rust programs and generates
cryptographic proofs of correct execution.

**Proof generation pipeline:**

1. The inference logic (from `zkml-prover::inference`) is compiled as a
   RISC Zero guest program targeting the RISC-V instruction set.
2. The host program provides the model and inputs to the guest via
   standard I/O.
3. The guest executes the inference, writes the result and input/model
   hashes to the journal (public outputs), and exits.
4. The zkVM produces a STARK proof of correct execution.
5. The STARK proof is compressed into a Groth16 SNARK using the Bonsai
   proving service or a local prover.
6. The resulting proof (three BN254 curve points) and journal data are
   submitted to the Soroban verifier contract.

**Key dependency:** `stellar-zk` by Nethermind provides the reference
Groth16 verifier contract for RISC Zero proofs on Soroban.

### Phase 2: Native Circuits

For Phase 2, the inference computation is expressed directly as arithmetic
constraints using a circuit framework such as `bellman` or `halo2`.

**Decision tree circuit design:**

- Each tree node is encoded as a conditional constraint.
- The prover provides the traversal path as a witness (sequence of
  binary selectors).
- The circuit enforces that each selector correctly reflects the
  comparison between the feature value and the node threshold.
- A path aggregation constraint ensures that exactly one leaf is
  selected.
- The output constraint binds the circuit output to the selected
  leaf's value.

**Advantages over zkVM approach:**

- Proof size: ~100-200 bytes vs ~300-500 bytes.
- Prover time: dedicated circuits avoid the overhead of simulating a
  full RISC-V processor.
- Verifier cost: fewer public inputs and simpler verification keys
  reduce on-chain gas.

---

## Verifier Contract Design

The Soroban verifier contract follows these design principles:

1. **Minimal state**: Only three storage entries (model hash, last result,
   initialization flag). This minimizes rent costs and attack surface.

2. **Single entry point for verification**: The `verify_inference` function
   accepts the proof and public inputs as raw bytes, performs the pairing
   check, and records the result atomically.

3. **No proof storage**: Proof data is consumed during verification and
   not persisted. Only the result and metadata are stored.

4. **Initialization guard**: The contract enforces single initialization
   to prevent model substitution attacks.

### Future Enhancements

- **Multi-model support**: Allow registering multiple model commitments
  with distinct identifiers.
- **Access control**: Role-based permissions for model registration and
  result querying.
- **Event emission**: Emit Soroban events on successful verification for
  off-chain indexing.
- **Batch verification**: Accept multiple proofs in a single transaction
  for throughput optimization.

---

## Benchmarking Considerations

The following metrics will be tracked across development phases:

| Metric                  | Phase 1 Target | Phase 2 Target | Unit    |
|-------------------------|----------------|----------------|---------|
| Proof generation time   | < 30s          | < 5s           | seconds |
| Proof size              | < 500          | < 200          | bytes   |
| Verification gas cost   | TBD            | 50% of Phase 1 | gas units|
| End-to-end latency      | < 60s          | < 15s          | seconds |
| Quantization accuracy   | > 99%          | > 99%          | %       |

Protocol 26 (Yardstick) benchmarking tools will be used to measure
on-chain verification costs precisely.

## Proof System Details

Phase 1 proves inference with the RISC Zero zkVM: the inference code runs as a
guest program, producing a STARK receipt that is then wrapped into a Groth16
SNARK. The Groth16 proof is what the Soroban contract verifies, because BN254
pairing checks (CAP-0074) make on-chain Groth16 verification cheap.

The public inputs bind the proof to a specific model and input through Poseidon
commitments (CAP-0075), so a valid proof cannot be replayed against a different
model or input. Phase 2 replaces the zkVM with native BN254 and Poseidon
circuits for smaller proofs and lower proving cost.
