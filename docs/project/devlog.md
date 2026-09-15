---
title: "Devlog"
description: "Chronological log of development decisions and progress."
icon: "clock-rotate-left"
---

Newest entries are at the bottom.

## Week 1 (April 2026)

Kicked off the scaffold. The workspace is split into three crates so the
off-chain prover and the on-chain verifier share types through `zkml-common`
without pulling Soroban dependencies into the prover or RISC Zero dependencies
into the contract.

## Week 2 (May 2026)

Built the `zkml-common` numeric core: fixed-point addition, subtraction,
multiplication, and saturating variants with tests. Added `ZkmlError`, a
quantized ReLU, and a minimal `Tensor` for dense layers.

**Note on matmul accumulation.** Dense layers accumulate after a per-product
right shift. For the tiny models targeted (a few dozen neurons per layer) this
stays well inside `i64`.

## Week 4 (May 2026)

The off-chain pipeline ran end to end for all model families: JSON import,
quantization, inference, commitment, and `VerificationBundle` assembly. The
verifier parsed public inputs, recorded results, counted verifications, and
emitted an event. The real cryptography remained.

## End of May 2026

Hardened the numeric core (checked add, sub, mul, div, dot product, hex helpers),
added batch inference, and set up repository hygiene: CI, issue and PR
templates, CODEOWNERS, a threat model, and security notes.

## June 2026

Feature scaling, a model commitment line in the CLI, tensor accessors, a
quantization report, validated inference, and bundle serialization. Released
`v0.2.0` on 2026-06-17 as the Phase 1 off-chain pipeline milestone. Next steps
at the time: RISC Zero proving, the BN254 pairing check, Poseidon commitments,
and a testnet guide.

## July 2026: contributor campaign, core cryptography

The project opened its issue backlog to external contributors (GrantFox OSS,
FWC26 campaign). Merged:

- Complete Q16.16 arithmetic, comparisons, and operators; proptest suite for
  fixed-point algebra.
- ONNX importer foundation: protobuf decode, per-domain opset floors, operator
  allowlist.
- RISC Zero guest program running the shared inference engine, with host
  receipt verification and journal cross-checks in dev mode.
- Golden vector test suite; TinyMLP inference with quantized ReLU.
- Extraction of `LinearClassifier` and `TreeEnsembleClassifier` from ONNX.
- Quantization validation: range checks, static overflow bounds, accuracy report.
- Poseidon commitments over BN254 (`light-poseidon`, circomlib parameters).
- Real Groth16 verification in the contract with BN254 host functions.

## August 2026: hardening the verifier and the pipeline

- Public inputs read little-endian to match the commitment scheme.
- Domain-separated Merkle hashing with inclusion proofs.
- Checked arithmetic in logistic regression; decision tree termination
  guarantees (cycle and reachability checks, bounded traversal).
- Accept-path Groth16 fixture and a resource budget harness
  (about 29.3M CPU instructions per verification).
- Generalized public-input-to-scalar mapping with canonical length checks.
- `no_std`-clean `zkml-common`.
- CLI subcommands `commit`, `infer`, `prove`, `validate`, `inspect` with strict
  input parsing.
- TinyMLP extraction from ONNX (`Gemm`, `MatMul` + `Add`, `Relu`).
- Decision layer: `class_label` public input (threshold for logistic
  regression, argmax for MLPs); layout fixed at 80 bytes, `VERSION` 5.
- Replay protection with a SHA-256 nullifier in persistent storage.
- Admin access control, key rotation, and pause.
- Instance TTL extension on `initialize` and successful verification.
- `verified` event enriched with `model_hash` (topic) and `output` (data).
- Property and differential (native vs zkVM) inference tests; per-row fallible
  batch inference; stable lowest-index argmax.
- Scaffolding for STARK-to-Groth16 (`groth16`, `bonsai` features) and the KYC
  demo (dataset, training, deploy script, demo runner skeleton).

**Status at end of August.** Everything off-chain up to the STARK receipt works,
and the contract verifies Groth16 proofs. The gap is the Groth16 wrap, the
verification key export, and how Route A public inputs map to the contract.

## September 2026: professionalization

- Documentation moved to a Mintlify site with accurate references, a known
  limitations page, and deployment and KYC guides.
- Verifier built for `wasm32v1-none` with a dedicated `contract` profile.
- Crate versions reset to `0.0.1` for the first crates.io releases of
  `zkml-common` and `zkml-verifier`, with a publish workflow.
- stellar-build skills added to the repository for contributor tooling.

**Next.** Route A verification path, Groth16 compression, VK export, and a real
testnet KYC demo.
