# zkml-verifier

Soroban smart contract that verifies Groth16 proofs of machine learning inference on Stellar,
part of [zkml-soroban](https://github.com/ZKML-Soroban/ZKML-Soroban).

It uses the BN254 host functions introduced in Stellar Protocol 25 (CAP-0074) to run the
Groth16 pairing check on-chain and records the verified result.

## Requirements

| Requirement | Version |
| ----------- | ------- |
| Stellar protocol | 25 (X-Ray) or later |
| `soroban-sdk` | 27 |
| Rust | 1.91 or later (required by `soroban-sdk` 27) |
| Build target | `wasm32v1-none` |

## Build

```bash
rustup target add wasm32v1-none
cargo build -p zkml-verifier --target wasm32v1-none --profile contract
# output: target/wasm32v1-none/contract/zkml_verifier.wasm
```

The `contract` profile is defined in the workspace root `Cargo.toml` (size-optimized, LTO,
overflow checks on).

The crate also builds as an `rlib` so host code and tests can use its types and the generated
contract client. Do not depend on it from another contract crate built for WASM: that would
pull the verifier's exported functions into your contract.

## Interface

Contract interface `VERSION` is `5`.

| Function | Auth | Description |
| -------- | ---- | ----------- |
| `initialize(admin, model_hash, vk)` | `admin` | Stores admin, model commitment and verification key. Callable once. |
| `verify_inference(proof_a, proof_b, proof_c, public_inputs)` | none | Verifies the proof, rejects replays, records the result, emits `verified`. |
| `get_result()` | none | Last `InferenceRecord`. |
| `get_model_hash()` | none | Registered model commitment. |
| `get_verification_count()` | none | Number of successful verifications. |
| `get_admin()` / `is_paused()` / `version()` | none | Read-only state. |
| `set_verification_key(vk)` | admin | Rotates the verification key. |
| `set_model_hash(model_hash)` | admin | Replaces the model commitment. |
| `set_admin(new_admin)` | admin | Transfers admin rights. |
| `set_pause(paused)` | admin | Pauses or resumes verification. |

### Proof encoding

| Argument | Size | Encoding |
| -------- | ---- | -------- |
| `proof_a` | 64 bytes | G1 point, `x \|\| y`, big-endian |
| `proof_b` | 128 bytes | G2 point, big-endian Ethereum layout |
| `proof_c` | 64 bytes | G1 point, `x \|\| y`, big-endian |

### Public inputs (80 bytes)

| Offset | Size | Field | Encoding |
| ------ | ---- | ----- | -------- |
| 0 | 32 | `model_hash` | Poseidon commitment, little-endian field element |
| 32 | 32 | `input_hash` | Poseidon commitment, little-endian field element |
| 64 | 8 | `output` | `i64` little-endian (raw Q16.16 value) |
| 72 | 8 | `class_label` | `i64` little-endian |

Each field is mapped to one BN254 scalar, so the verification key must contain exactly
5 `IC` points (`IC[0]` plus one per public input).

## Verification equation

```
e(-A, B) * e(alpha, beta) * e(L, gamma) * e(C, delta) == 1
L = IC[0] + sum(x_i * IC[i + 1])
```

## Errors

| Code | Variant |
| ---- | ------- |
| 1 | `ContractNotInitialized` |
| 2 | `PublicInputsTooShort` |
| 3 | `MalformedProofA` (also returned for a malformed `proof_c`) |
| 4 | `MalformedProofB` |
| 5 | `MalformedProofC` (reserved, not returned by VERSION 5) |
| 6 | `MalformedVerificationKey` |
| 7 | `VerificationFailed` (also returned while paused) |
| 8 | `InvalidPublicInputLength` |
| 9 | `VerificationKeyLengthMismatch` |
| 10 | `ProofAlreadyUsed` |

## Storage

- Instance storage: admin, model hash, verification key, last result, counter, pause flag.
  TTL is extended on `initialize` and on each successful verification (30-day threshold,
  120-day extension).
- Persistent storage: one nullifier per verified public-input set (`sha256(public_inputs)`),
  extended to the network maximum TTL.

## Cost

A full `verify_inference` (4 scalar multiplications plus a 4-pair pairing check) measures about
29.3M CPU instructions and 278 KB of memory in the Soroban test budget. See
`test_verifier_accept_path_and_resource_budget`.

## Known limitations

- Proofs produced by the RISC Zero Groth16 wrapper expose RISC Zero's own public inputs
  (control root and claim digest), not this 80-byte layout. Verifying them requires
  deriving the claim digest from the journal on-chain; this is tracked on the roadmap.
- A given `(model, input, output, class_label)` tuple can be verified only once.
- `get_result` returns a single global slot that the next successful verification overwrites.

## License

Apache-2.0
