---
title: "Known limitations"
description: "Honest list of what does not work yet and design gaps under review."
icon: "circle-exclamation"
---

zkml-soroban is pre-production. This page lists current gaps so integrators can
plan around them. Each item is tracked on the [roadmap](/project/roadmap).

## Proving

- **No Groth16 proofs yet.** STARK-to-Groth16 compression
  (`compress_to_groth16_local`, `compress_to_groth16_bonsai`) returns
  "not yet implemented", and `generate_proof` emits an empty proof.
- **Bonsai backend is obsolete.** RISC Zero shut down Bonsai in December 2025. The
  `bonsai` feature is a legacy stub to be replaced by Boundless or a self-hosted Bento prover.
- **Local Groth16 wrapping is x86_64 Linux only** and requires Docker.
- **No verification key export.** There is no tool to produce the
  `VerificationKey` for `initialize`.
- **Route A public inputs.** The contract uses
  `(model_hash, input_hash, output, class_label)` as Groth16 public inputs. A
  Groth16 proof produced by RISC Zero's wrapper is verified against RISC Zero's
  universal verification key, whose public inputs are derived from the control
  root and the claim digest (which commits to the image ID and the journal
  hash). Verifying Route A proofs requires a contract path that reconstructs the
  claim digest from the journal. The current layout fits native circuits
  (Route B) directly.
- **Panicking decision path.** `run_inference_with_decision` can panic (MLP
  overflow, tree iteration limit), so `generate_proof` does not return those
  failures as `Err`.

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
