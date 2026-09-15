---
title: "Proving pipeline"
description: "How inference is proven with the RISC Zero zkVM, and what is still pending."
icon: "shield-check"
---

Phase 1 proves inference with the RISC Zero zkVM. The guest produces a STARK
receipt and a public journal. Wrapping that receipt into a Groth16 proof for
Soroban is not implemented yet.

## Status

| Step                                          | Status        |
| --------------------------------------------- | ------------- |
| Commitments (`model_commitment`, `input_commitment`) | Implemented |
| Guest execution and STARK receipt (`generate_receipt`) | Implemented, CI in dev mode |
| Journal cross-check against native inference  | Implemented   |
| STARK to Groth16 (local Docker)                | Pending (functions return an error) |
| Verification key export                        | Pending       |
| `VerificationBundle` with real proof bytes     | Pending (`generate_proof` emits an empty proof) |

<Warning>
RISC Zero shut down the Bonsai proving service in December 2025. The `bonsai` feature and
`ProverBackend::Bonsai` are legacy stubs and will be replaced by Boundless or a self-hosted
Bento prover. Local Groth16 wrapping requires x86_64 Linux with Docker.
</Warning>

## Pinned versions

| Component                                  | Version  |
| ------------------------------------------ | -------- |
| `risc0-zkvm`, `risc0-build`                | `=3.0.6` |
| `risc0-groth16` (feature `groth16`)        | `=3.0.5` |
| `bonsai-sdk` (feature `bonsai`)            | `1.1`    |
| `cargo-risczero`, `r0vm` toolchain         | `3.0.6`  |

Exact pins reduce the risk from RISC Zero API changes.

## Feature flags (`zkml-prover`)

| Feature   | Enables                                                          |
| --------- | ---------------------------------------------------------------- |
| `zkvm`    | `generate_receipt`, `decode_journal` (needs the RISC Zero toolchain) |
| `groth16` | `generate_proof_with_groth16` with `ProverBackend::Local` (stub)  |
| `bonsai`  | `ProverBackend::Bonsai` (stub)                                    |
| `timing`  | Timing output for inference and proving steps                     |

## Steps

1. Compute the model and input commitments.
2. Build an executor environment with the `Model` and the input vector.
3. Prove guest execution with `default_prover()`.
4. Verify the receipt against `ZKML_GUEST_ID`.
5. Decode the journal and cross-check every field against native
   `run_inference_with_decision` and the native commitments.
6. *(Pending)* Wrap the receipt into Groth16 and serialize A, B, C.
7. Package proof and public inputs into a `VerificationBundle`.

## Guest journal

Committed by `methods/guest/src/main.rs`:

| Field         | Type       | Meaning                                  |
| ------------- | ---------- | ---------------------------------------- |
| `model_hash`  | `[u8; 32]` | `commitment_hash(model_elements(model))` |
| `input_hash`  | `[u8; 32]` | `commitment_hash(input raw values)`      |
| `output`      | `i64`      | Raw `FixedPoint::value` of the score      |
| `class_label` | `i64`      | Decision (see [Models](/concepts/models)) |

## Dev mode and real proving

| Mode | Environment           | Use                                  |
| ---- | --------------------- | ------------------------------------ |
| Dev  | `RISC0_DEV_MODE=1`    | CI and local tests (fake receipts)   |
| Real | unset or `0`          | Manual, `#[ignore]` tests only       |

```bash
RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm
```

## Code layout

```text
crates/zkml-common   models, FixedPoint, inference, commitments
methods/guest        zkVM guest (depends only on zkml-common)
crates/zkml-prover   host: generate_receipt, generate_proof, bundles
```

## Known limitations

- `generate_proof` returns a bundle with empty proof bytes.
- `run_inference_with_decision` can panic (MLP overflow, tree iteration limit),
  so `generate_proof` does not surface those failures as `Err` yet.
- A RISC Zero Groth16 proof is verified against RISC Zero's universal
  verification key, whose public inputs are derived from the image ID and the
  journal digest, not the four journal fields directly. The current contract
  layout needs an adapter for Route A. See
  [Known limitations](/security/known-limitations).
- Model and input commitments share domain `0` in the proving path.
