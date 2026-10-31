---
title: "Known limitations"
description: "Honest list of what does not work yet and design gaps under review."
icon: "circle-exclamation"
---

zkml-soroban is pre-production. This page lists current gaps so integrators can
plan around them. Each item is tracked on the [roadmap](/project/roadmap).

## Proving

- **Nothing has been verified on a live network yet.** `verify_receipt` accepts
  a real receipt in the Soroban test environment, at 29.6 million CPU
  instructions on the compiled WASM against a 100 million limit, but the
  contract has not been deployed to testnet and no transaction has carried a
  proof.
- **`verify_inference` still traps on a corrupted proof point.** `verify_receipt`
  validates its points and returns a typed error; the native-circuit path does
  not yet. In both, a G2 point on the curve but outside the subgroup traps,
  which takes a deliberately crafted point.
- **Groth16 compression needs x86_64 Linux with Docker.** The Circom witness
  generator image is not published for other platforms (risc0 issue #1749), so
  `prove_groth16` fails fast with `UnsupportedPlatform` elsewhere. The `cuda`
  feature wraps natively on Linux with an NVIDIA GPU in theory, but the native
  wrap crashed on the one card it was tried on; see
  [Proving](/concepts/proving).
- **The guest image id depends on the build machine.** `methods/build.rs`
  compiles the guest with the host toolchain, so the same commit produces a
  different image id on different machines, and a contract only accepts
  receipts from the id its admin registered. Register the id that
  `zkml-prover export-vk` prints on the machine that produces the proofs, until
  the guest is built with RISC Zero's reproducible Docker build.
- **No remote proving.** RISC Zero shut down Bonsai in December 2025.
  `--backend boundless` returns `ProveError::RemoteBackend` until a Boundless or
  self-hosted Bento client is written.
- **Dev mode produces no proof.** `RISC0_DEV_MODE=1` makes receipts fake, so
  `prove_groth16` refuses to run under it rather than emitting a bundle that
  looks real.
- **The legacy `prove` output carries no proof.** It stays for inspection and
  prints a warning; it must never be accepted as evidence.
- **Panicking decision path.** `run_inference_with_decision` can panic (MLP
  overflow, tree iteration limit). The fallible
  `try_run_inference_with_decision` is what the proving path uses, but the
  panicking function is still public.

## Commitments

- The proving path uses `commitment_hash` (domain `0`) for both model and input
  commitments; the domain-separated `commit_model` (1) and `commit_inputs` (2)
  are not wired in.
- The input commitment does not absorb the input length: inputs are padded
  with zeros to the hash rate, so `[x]` and `[x, 0]` commit to the same value.
  The model commitment is length-prefixed since `0.0.2`.
- `bundle_id` ignores `class_label`.

## Models

- Decision trees always produce `class_label = 0`; the prediction is in `output`.
- ONNX: single trees only (no ensembles), binary linear classifiers only.
- ONNX trees with text class labels are refused; export the classifier with
  integer labels.

## Contract

- **One verification per tuple.** The nullifier is `sha256(public_inputs)`, so a
  legitimate re-check of the same input and result is rejected with
  `ProofAlreadyUsed`.
- **Single result slot.** `get_result` returns a global last record that any
  successful verification overwrites; there is no per-subject lookup.
- **One model per contract.** A contract holds a single model commitment; a
  second model needs a second deployment.
- **Pause error.** A paused contract returns the generic `VerificationFailed`.
- **Some getters panic.** `get_risc0_config` and `get_risc0_vk` panic until the
  RISC Zero values are set. `get_admin` panics only on a contract without stored
  state, which the constructor rules out.
- **A native key with a known trapdoor cannot be detected.** The constructor and
  `set_verification_key` refuse a native key with a point off its curve or at
  infinity, but a key built from points whose discrete logs are known, such as
  generator points, passes and lets anyone prove any public input against it.
  There is no native circuit yet, so deploy without a native key; see
  [Testnet deployment](/guides/deployment).

## Tooling

- `zkml-demo` is a skeleton.
- `examples/kyc-demo/deploy.sh` still uses the deprecated `soroban` CLI, calls
  `initialize`, which the constructor replaced, and passes all-zero placeholder
  keys, which the constructor now refuses. Use the
  [deployment guide](/guides/deployment).
- **Windows needs a larger main thread stack.** Windows gives the main thread
  1 MB where Linux and macOS give 8 MB, and the Poseidon commitment path
  overflows it in an unoptimized build, so `zkml-prover commit` exits with
  `0xC00000FD`. Inside this workspace `.cargo/config.toml` raises it to 8 MB for
  the MSVC targets. Anything building these crates outside the workspace has to
  carry the same linker flag or build with `--release`.

## Cryptographic horizon

BN254 pairings and Groth16 are not secure against large-scale quantum
computers. Long-lived attestations should account for this.
