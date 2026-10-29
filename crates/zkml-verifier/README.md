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

Contract interface `VERSION` is `10`. The full reference, with argument types and the
order of checks, is in [docs/reference/verifier-contract.md](https://github.com/ZKML-Soroban/ZKML-Soroban/blob/main/docs/reference/verifier-contract.md).

| Function | Auth | Description |
| -------- | ---- | ----------- |
| `__constructor(admin, model_hash, vk)` | `admin` | Runs once, atomically with deploy. Stores admin, model commitment and, if given, the native verification key (`Option`). Without one, `verify_inference` is off. There is no public `initialize`. |
| `verify_inference(proof_a, proof_b, proof_c, public_inputs)` | none | Verifies a native Groth16 proof, rejects replays, records the result, emits `verified`. |
| `verify_receipt(seal, journal)` | none | Verifies a RISC Zero Groth16 receipt, rejects replays, records and returns the result, emits `verified`. |
| `claim_digest(image_id, journal)` | none | Recomputes the digest a receipt commits to, for off-chain checks. Returns `MalformedJournal` for a journal that is not 96 bytes. |
| `get_result()` | none | Last `Option<InferenceRecord>`. |
| `get_model_hash()` | none | Registered model commitment (`Option<Bytes>`). |
| `get_verification_count()` | none | Number of successful verifications. |
| `get_admin()` / `is_paused()` / `version()` | none | Read-only state. |
| `get_risc0_config()` / `get_risc0_vk()` | none | Registered RISC Zero configuration and verifying key. |
| `set_verification_key(vk)` | admin | Rotates the verification key for `verify_inference`. |
| `set_model_hash(model_hash)` | admin | Replaces the model commitment. |
| `set_risc0_config(config)` / `set_risc0_vk(vk)` | admin | Registers the guest image id, control root, selector and RISC Zero's verifying key. |
| `propose_admin(new_admin)` | admin | Starts a two-step admin transfer. |
| `accept_admin()` | pending admin | Completes the transfer. |
| `set_pause(paused)` | admin | Pauses or resumes verification. |

Every admin setter emits an event. Events are declared with `#[contractevent]`, so their
schemas ship in the contract spec; see the events section of the contract reference.

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
| 3 | `MalformedProofA` (in `verify_inference`, also returned for a wrong-length `proof_c`) |
| 4 | `MalformedProofB` |
| 5 | `MalformedProofC` (returned by `verify_receipt`) |
| 6 | `MalformedVerificationKey` |
| 7 | `VerificationFailed` (also returned while paused) |
| 8 | `InvalidPublicInputLength` |
| 9 | `VerificationKeyLengthMismatch` |
| 10 | `ProofAlreadyUsed` |
| 11 | `UnknownSelector` |
| 12 | `MalformedSeal` |
| 13 | `MalformedJournal` |
| 14 | `Risc0NotConfigured` |
| 15 | `NonCanonicalPublicInput` |
| 16 | `NativeKeyNotConfigured` |

Causes for each code are in [docs/reference/errors.md](https://github.com/ZKML-Soroban/ZKML-Soroban/blob/main/docs/reference/errors.md).

## Storage

- Instance storage: admin, pending admin, model hash, verification keys, RISC Zero
  configuration, last result, counter, pause flag. TTL is extended in the constructor and on
  each successful verification (30-day threshold, 120-day extension).
- Persistent storage: one nullifier per verified public-input set (`sha256(public_inputs)`),
  extended to the network maximum TTL.

## Cost

Measured on the compiled WASM, `verify_inference` (4 scalar multiplications plus a
4-pair pairing check) costs 30.2M CPU instructions and `verify_receipt` 29.6M,
against a 100M limit per transaction. Measure on the WASM rather than natively:
the native test environment meters only host calls. See the `wasm_budget` tests.

## Known limitations

- RISC Zero receipts go through `verify_receipt`, which derives the claim digest from the
  journal on-chain. Neither entrypoint has been exercised by a deployed contract on a live
  network yet.
- `verify_inference` does not validate its proof points, so a corrupted one traps in the
  host there. `verify_receipt` does.
- A given `(model, input, output, class_label)` tuple can be verified only once.
- `get_result` returns a single global slot that the next successful verification overwrites.

## License

Apache-2.0
