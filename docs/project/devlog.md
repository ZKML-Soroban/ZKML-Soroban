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

## September 2026: real proofs

STARK to Groth16 compression works. `prove --groth16` runs the model in the
zkVM, compresses the receipt, and writes a bundle with a real 260-byte seal.
`verify-bundle` checks it with RISC Zero's own verifier, and `export-vk` prints
the constants the contract will need.

Three decisions shaped the work:

**The journal became a fixed 96-byte record.** The guest used to commit a serde
struct. A Groth16 receipt binds the journal through a claim digest computed over
exactly the committed bytes, so anything whose encoding can drift between
versions is a liability. `JournalV1` has a magic prefix, a version field and
fixed offsets, and the contract's 80-byte public inputs are derived from it.

**The RISC Zero digest arithmetic was reimplemented in `zkml-common`.** The
contract cannot depend on `risc0-zkvm`, but it has to recompute the claim digest
from the journal it is handed, otherwise it is verifying that *some* program
produced *some* output. `zkml_common::risc0` does that in `no_std`, and
`tests/risc0_digests.rs` cross-checks every value against the real crates. That
test earned its place immediately: it caught that `bn254_control_id` must be
byte-reversed before it is read as a field element, which would have made every
pairing check fail with nothing to point at.

**The `bonsai` feature was deleted rather than kept as a stub.** RISC Zero shut
Bonsai down in December 2025. A feature flag that cannot work is worse than no
flag, so the dependency and the code are gone; `--backend boundless` returns a
named error until a client exists.

Local compression needs x86_64 Linux with Docker (the Circom witness generator
image is not published elsewhere), so the platform is checked before any
expensive work starts and CI runs the real thing only on demand.

**Next.** The contract side: reconstruct the claim digest on chain, build the
five public inputs and verify these bundles on Stellar (issue #84).

## September 2026: verified on chain

`verify_receipt` accepts a real RISC Zero receipt inside the contract, at
30,677,367 CPU instructions against a 100 million limit, which is 4.7% more
than the native-circuit path. The receipt was regenerated from the guest this
code builds, so the test proves this repository's output rather than a stored
artefact.

The work mostly consisted of not breaking things quietly:

**The contract could not link zkml-common.** Poseidon pulled in arkworks, whose
`num-traits` does not build for `wasm32v1-none`, and serde came in with its
std layer because the workspace enabled default features. The crate had been
documented as `no_std` and was not. Poseidon now sits behind a feature, and
serde's features are decided at the workspace root.

**Two byte-order traps were caught by reading, not by failing.** The existing
scalar conversion reads little-endian, but RISC Zero's public inputs are
big-endian, so receipts got their own. The seal's curve points, on the other
hand, turned out to be stored in exactly the order Soroban reads, so they pass
through untouched. Either mistake would have produced a well-formed scalar or
point that fails every pairing with nothing to point at.

**One behaviour is documented rather than fixed.** A corrupted curve point traps
in the host instead of returning an error. Validating points up front would
cost gas on every honest verification to improve the message on a dishonest
one.

**Next.** Deploy to testnet and verify a receipt in a real transaction.
