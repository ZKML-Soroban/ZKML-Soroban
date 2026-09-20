---
title: "Proving pipeline"
description: "How inference is proven with the RISC Zero zkVM and compressed to a Groth16 proof."
icon: "shield-check"
---

Inference is proven with the RISC Zero zkVM. The guest runs the model and
commits a journal; the host proves that execution, compresses the STARK receipt
to a Groth16 proof, and packages the result as a verification bundle.

## Status

| Step                                                   | Status                                  |
| ------------------------------------------------------ | --------------------------------------- |
| Commitments (`model_commitment`, `input_commitment`)    | Implemented                             |
| Guest execution and STARK receipt (`generate_receipt`)  | Implemented, CI in dev mode             |
| Journal cross-check against native inference            | Implemented                             |
| STARK to Groth16 (`prove_groth16`)                      | Implemented (x86_64 Linux with Docker)  |
| Verification key export (`export-vk`)                   | Implemented                             |
| `VerificationBundleV2` with real proof bytes            | Implemented                             |
| On-chain verification of those bundles                  | Pending (issue #84)                     |
| Remote proving (Boundless)                              | Not implemented (returns an error)      |

<Warning>
Groth16 compression shells out to a Docker image that runs the Circom witness
generator, and that image is published for x86_64 only (risc0 issue #1749). On
any other platform `prove_groth16` fails immediately with
`ProveError::UnsupportedPlatform` instead of failing deep inside the prover.
RISC Zero shut down the Bonsai proving service in December 2025, so there is no
hosted fallback; the remote path is reserved for Boundless.
</Warning>

## Pinned versions

| Component                           | Version  |
| ----------------------------------- | -------- |
| `risc0-zkvm`, `risc0-build`         | `=3.0.6` |
| `cargo-risczero`, `r0vm` toolchain  | `3.0.6`  |

Exact pins reduce the risk from RISC Zero API changes. Compression itself lives
in `risc0-zkvm` (`ProverOpts::groth16()`), so there is no separate pin for it.

## Feature flags (`zkml-prover`)

| Feature   | Enables                                                                |
| --------- | ---------------------------------------------------------------------- |
| `zkvm`    | `generate_receipt`, `decode_journal`, `verify_bundle`, `export-vk`      |
| `groth16` | `prove_groth16` and `prove --groth16` (implies `zkvm`)                  |
| `timing`  | Timing output for inference and proving steps                          |

## Steps

1. Compute the model and input commitments.
2. Build an executor environment with the `Model` and the input vector.
3. Prove guest execution with `default_prover().prove_with_opts(.., ProverOpts::groth16())`.
4. Verify the receipt against `ZKML_GUEST_ID`.
5. Decode the journal and cross-check every field against native inference and
   the native commitments. A mismatch is `ProveError::JournalMismatch`.
6. Encode the seal: the 4-byte selector followed by the 256-byte proof.
7. Package everything into a [`VerificationBundleV2`](/reference/bundle-format).

```bash
# Real proof, written to disk.
cargo run -p zkml-prover --features groth16 -- \
  prove examples/models/credit_lr.json -i 0.5,0.2,0.9,0.1 --groth16 -o bundle.json

# Verify it locally, with RISC Zero's own verifier.
cargo run -p zkml-prover --features zkvm -- verify-bundle bundle.json
```

## Guest journal

Committed by `methods/guest/src/main.rs` as a fixed 96-byte record
(`JournalV1`), so the layout is stable across versions:

| Field         | Type       | Meaning                                  |
| ------------- | ---------- | ---------------------------------------- |
| `model_kind`  | `u8`       | Which model family ran                    |
| `model_hash`  | `[u8; 32]` | `commitment_hash(model_elements(model))` |
| `input_hash`  | `[u8; 32]` | `commitment_hash(input raw values)`      |
| `output`      | `i64`      | Raw `FixedPoint::value` of the score      |
| `class_label` | `i64`      | Decision (see [Models](/concepts/models)) |

## What a Groth16 receipt actually proves

A RISC Zero Groth16 proof is verified against RISC Zero's universal verifying
key. Its five public inputs are not the journal fields:

```text
[control_root_lo, control_root_hi, claim_digest_lo, claim_digest_hi, bn254_control_id]
```

`claim_digest` is the digest of `ReceiptClaim::ok(image_id, journal)`, which
binds *which program ran* to *what it output*. A verifier that holds the journal
therefore has to recompute that digest. `zkml_common::risc0` implements that
arithmetic without depending on `risc0-zkvm`, so the `no_std` contract can do it
too; `crates/zkml-prover/tests/risc0_digests.rs` cross-checks every value
against the real RISC Zero crates.

Two details are easy to get wrong and are covered by those tests: the digests
are split into 128-bit halves after being byte-reversed, and `bn254_control_id`
is byte-reversed before it is read as a field element.

## Dev mode and real proving

| Mode | Environment        | Use                                   |
| ---- | ------------------ | ------------------------------------- |
| Dev  | `RISC0_DEV_MODE=1` | CI and local tests (fake receipts)    |
| Real | unset or `0`       | Manual, `#[ignore]` tests, releases   |

```bash
RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm
```

Dev mode produces no seal at all, so `prove_groth16` refuses to run under it
(`ProveError::DevModeHasNoSeal`) rather than emitting a bundle that looks real.

## Code layout

```text
crates/zkml-common   models, FixedPoint, inference, commitments, journal, risc0, bundle
methods/guest        zkVM guest (depends only on zkml-common)
crates/zkml-prover   host: generate_receipt, prove_groth16, verify_bundle, bundles
```

## Known limitations

- The contract does not verify these bundles yet; that is issue #84.
- Remote proving (`--backend boundless`) returns `ProveError::RemoteBackend`.
- Compression needs roughly 10 GB of RAM and takes minutes on a laptop.
- Model and input commitments share domain `0` in the proving path.
- The legacy `prove` output (without `--groth16`) carries public inputs but no
  proof. It prints a warning and must not be treated as evidence.
