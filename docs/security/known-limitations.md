---
title: "Known limitations"
description: "Honest list of what does not work yet and design gaps under review."
icon: "circle-exclamation"
---

zkml-soroban is pre-production. This page lists current gaps so integrators can
plan around them. Each item is tracked on the [roadmap](/project/roadmap).

## Proving

- **Nothing has been verified on a live network yet.** `verify_receipt` accepts
  a real receipt in the Soroban test environment, at 30.7 million CPU
  instructions against a 100 million limit, but the contract has not been
  deployed to testnet and no transaction has carried a proof.
- **A corrupted curve point traps rather than returning an error.** The BN254
  host functions abort the transaction on a point they cannot parse. Nothing is
  verified either way, but the diagnostic is worse.
- **Groth16 compression is x86_64 Linux only** and requires Docker. The Circom
  witness generator image is not published for other platforms (risc0 issue
  #1749), so `prove_groth16` fails fast with `UnsupportedPlatform` elsewhere.
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
- Decision tree nodes are serialized without a node-type tag.
- `bundle_id` ignores `class_label`.

## Models

- Decision trees always produce `class_label = 0`; the prediction is in `output`.
- ONNX: single trees only (no ensembles), binary linear classifiers only.
- The KYC demo exports ONNX with opset 12, below the importer floor of 17.

## Contract

- **One verification per tuple.** The nullifier is `sha256(public_inputs)`, so a
  legitimate re-check of the same input and result is rejected with
  `ProofAlreadyUsed`.
- **Single result slot.** `get_result` returns a global last record that any
  successful verification overwrites; there is no per-subject lookup.
- **Pause error.** A paused contract returns the generic `VerificationFailed`.
- **Admin changes are not events.** Setters only log.
- **Some getters panic.** `get_result`, `get_model_hash` and `get_admin` panic before
  initialization or before the first result; `get_verification_count` and `is_paused` do not.
- **No point validation beyond the host.** Points at infinity are accepted if
  the host accepts them; test fixtures with zero G2 points make the pairing pass
  trivially.

## Tooling

- `zkml-demo` is a skeleton.
- `examples/kyc-demo/deploy.sh` still uses the deprecated `soroban` CLI, omits
  `--admin`, and uses placeholder values. Use the
  [deployment guide](/guides/deployment).

## Cryptographic horizon

BN254 pairings and Groth16 are not secure against large-scale quantum
computers. Long-lived attestations should account for this.
