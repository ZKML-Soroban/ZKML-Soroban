# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking
- The contract `VERSION` is bumped `5 -> 6` for `verify_receipt` and the RISC Zero
  configuration calls below.
- `zkml-common` built with `default-features = false` no longer includes the
  `commitment` and `merkle` modules unless the new `poseidon` feature is on.
  Their arkworks dependency does not build for `wasm32v1-none`, and gating it is
  what lets the contract link the crate. Default builds are unaffected.

### Added
- `verify_receipt(seal, journal)` verifies a RISC Zero Groth16 receipt on chain,
  against RISC Zero's universal verifying key, and returns the `InferenceRecord`
  it stores. A real receipt verifies at 29,614,411 CPU instructions on the
  compiled WASM, against a network limit of 100 million.
- Checks run cheapest first: the seal's length and selector, the journal's
  layout, the registered model hash, then replay, so a used receipt is refused
  before it costs a pairing. Only then are the proof points validated and the
  pairing run, and the nullifier is spent only on success.
- The three proof points are validated before the host sees them, so a
  corrupted seal returns `MalformedProofA`, `MalformedProofB` or
  `MalformedProofC` instead of aborting the transaction. A test flips each of the
  356 bytes of the seal and the journal and requires a typed error for every
  one. `zkml_common::bn254` checks the G2 curve equation, since Soroban has no
  host function for it, and is tested against `ark-bn254`.
- `set_risc0_config` / `get_risc0_config` for the guest image id, control root,
  BN254 control id and seal selector, as fixed-size `BytesN` fields.
  `set_risc0_config` refuses a control id that is not a valid BN254 scalar,
  which the host would otherwise reduce silently.
- `set_risc0_vk` / `get_risc0_vk` for RISC Zero's universal verifying key. That
  key is a second one, not a replacement: a receipt has five public inputs where
  the native-circuit path has four, so it carries six `ic` points. It refuses any
  other length, and any point off its curve or at infinity. A point at infinity
  would unbind the journal from the proof, so a seal could verify against any
  journal.
- Both setters publish a `cfg_upd` event, so a change to what verifies is public.
- `claim_digest` as a public view, so off-chain tools can check that they agree
  with the contract. It refuses a journal that is not 96 bytes.
- Error codes `UnknownSelector` (11), `MalformedSeal` (12), `MalformedJournal`
  (13) and `Risc0NotConfigured` (14).
- `zkml-prover export-vk` also prints RISC Zero's universal verifying key,
  derived from the pinned crate rather than copied, and `--format soroban-args`
  prints the two admin calls that register everything.
- A golden fixture under `crates/zkml-verifier/testdata/`, a real seal and
  journal, with 24 tests against it covering verification, replay, every
  single-byte change, each proof point, the field-modulus boundary, another
  model, another guest, another control root, a paused contract, an
  unconfigured one, and degenerate verifying keys.
- `wasm_budget` tests that measure both entry points on the compiled WASM,
  because the native test environment only meters host calls.
- STARK to Groth16 compression (`prove_groth16`, `prove --groth16`). The prover
  produces a real 260-byte seal instead of an empty placeholder. Local
  compression needs x86_64 Linux with Docker; the platform and Docker are
  checked before any expensive work starts.
- `VerificationBundleV2` (`zkml_common::bundle`): hex-encoded image id, seal,
  journal and provenance metadata, with a compact binary encoding and an
  `AnyBundle` parser that accepts both versions.
- `JournalV1` (`zkml_common::journal`): the guest commits a fixed 96-byte
  record, so the journal layout is versioned and the 80-byte contract public
  inputs are derived from it.
- `zkml_common::risc0`: RISC Zero digest arithmetic (tagged structs, claim
  digest, digest splitting, the five Groth16 public inputs) without the RISC
  Zero crates, with the hash function injected so the contract can use
  `env.crypto().sha256()` while the prover uses `sha2`. Cross-checked against
  `risc0-zkvm` in `tests/risc0_digests.rs`, over more than 200 journals.
- CLI: `verify-bundle` verifies a v2 bundle with RISC Zero's own verifier.
- `ProveError`, a typed error per recoverable failure of the proving pipeline,
  and `try_run_inference_with_decision`, its fallible inference entry point.
- A manual `workflow_dispatch` CI job that proves and verifies a real Groth16
  bundle.

### Changed
- `zkml-common` is genuinely `no_std` now, which it was documented as but was
  not: Poseidon sits behind the `poseidon` feature, and serde no longer pulls in
  its std layer. This is what lets the contract share the journal codec and the
  digest arithmetic instead of duplicating them.
- `prove` without `--groth16` prints a warning that the bundle carries no proof.
- The `bonsai` feature is gone, along with the `bonsai-sdk` and `risc0-groth16`
  dependencies. RISC Zero shut down Bonsai in December 2025; remote proving is
  reserved for Boundless and currently returns an error.
- `generate_proof` is deprecated in favour of `prove_groth16`.
- The crates.io dry run packages the crates together, so the verifier is
  checked against the `zkml-common` in the same checkout.

### Fixed
- The BN254 control id is byte-reversed before being used as a public input, as
  `risc0_groth16::Verifier::new` does. Without it every pairing check would
  fail.
- `tagged_struct` refuses more digests or data words than its fixed buffer holds,
  instead of silently hashing a truncated struct in release builds.
- `cargo test --workspace` passes on Windows: the main thread gets the same 8 MB
  of stack it has on Linux and macOS.

## [0.0.1] - 2026-09-15

First release published to crates.io (`zkml-common`, `zkml-verifier`). The
crates.io version line restarts at `0.0.x`; the earlier `v0.2.0` tag marks the
June 2026 Phase 1 off-chain milestone and was never published.

### Added
- Crate publishing workflow (`publish-crate.yml`) with a single workspace
  version, crates.io metadata and per-crate READMEs.
- Mintlify documentation site under `docs/`, including a post-quantum readiness page.
- Graphviz diagram sources under `diagrams/` rendered to `docs/diagrams/`.
- CI jobs for the verifier WASM build (`wasm32v1-none`), `no_std`, zkVM guest
  and documentation link checks.
- `try_run_batch` returns per-row `Result`s so one malformed row cannot
  abort the rest of a batch. `run_batch` stays the all-valid panicking path.
- Instance-storage TTL bump on `initialize` and on successful `verify_inference`,
  with named threshold/extend-to constants (30d / 120d).
- Enriched the `verified` event with `model_hash` (published as a topic) and
  `output` (published as data) so off-chain indexers can derive which model
  produced which output directly from the event stream. This is a breaking
  change to the event payload (`data` changed from a bare `u32` to a
  `(u32, Bytes)` tuple); the contract `VERSION` is bumped `2 -> 3`.
- Admin access control for the verifier contract with `set_admin`, `set_verification_key`,
  `set_model_hash`, and `set_pause` functions. The `initialize` function now requires
  an `admin` parameter, which is a breaking interface change; the contract `VERSION`
  is bumped `3 -> 4`.
- Property-based tests for inference (`try_run_inference` determinism, ReLU
  monotonicity, stable argmax, no panics) and a zkVM differential suite that
  compares `generate_receipt` journals against native inference.
- `zkml-prover` CLI subcommands (`commit`, `infer`, `prove`, `validate`,
  `inspect`) built on clap, with strict `--input` parsing that rejects
  unparseable, empty, non-finite, and out-of-range fields instead of silently
  dropping them. Replaces the positional `zkml-prover <model> "<inputs>"` form.
- TinyMLP fixed-point inference with checked Q16.16 dense layers, ReLU on
  hidden layers only, topology validation via `TinyMLP::validate` /
  `try_run_inference`, and prover acceptance tests (hand-computed 2→2→1,
  ReLU, shape errors, golden float bound).
- ONNX importer foundation: protobuf parse, per-domain opset validation
  (core >= 17, ai.onnx.ml >= 1), operator allowlist, and typed
  `OnnxImportError`.
- `model_io::import_json` for the JSON exchange path used by the CLI and demos.
- RISC Zero guest program and host `generate_receipt` with journal
  cross-checks (dev-mode CI; real proving documented and `#[ignore]`d).
- Shared inference engine in `zkml-common` for native and guest paths.
- `commitment_hash` abstraction over model/input binding (Poseidon: #13).
- Minimal golden vectors under `crates/zkml-prover/tests/vectors/`.
- `PartialOrd` / `Ord` for `FixedPoint` (same-scale raw integer comparison).
- `Add`, `Sub`, and `Mul` operators as panicking wrappers over checked arithmetic.
- `Neg` for `FixedPoint` so negation uses the standard unary operator.
- `FixedPoint::abs`, `is_zero`, and `signum` helpers.
- `FixedPoint::clamp` for range saturation.
- `sum` and `mean` slice reductions for pooling layers.
- `max`, `min`, and `argmax` slice reductions for max-pooling and classification.
- ZK-friendly `leaky_relu` activation with a power-of-two slope.
- `relu6` bounded activation for quantized networks.
- `hard_sigmoid` and `hard_swish` piecewise-linear activations.
- `hardtanh` bounded activation clamping to `[-1, 1]`.
- Element-wise `relu6_vec`, `hard_sigmoid_vec`, and `hard_swish_vec` helpers.
- Initial workspace scaffold with `zkml-common`, `zkml-prover`, and
  `zkml-verifier` crates.

### Changed
- The verifier WASM is built with the workspace `contract` profile and the
  `wasm32v1-none` target. The previous per-crate release profile was ignored
  by Cargo.
- **Breaking:** verifier public-input layout is now fixed at commitment(64) + output(8) + class_label(8) = 80 bytes, and InferenceRecord gains class_label. Contract VERSION 4 -> 5.
- `prover::generate_proof` computes the output and class label with
  `run_inference_with_decision`. It still panics on invalid input or overflow
  (tracked in the known limitations).

## [0.2.0] - 2026-06-17

### Added
- Fixed-point checked/saturating arithmetic, division, and dot product.
- Quantized ReLU activation, `Tensor` type, and model validation helpers.
- Model and input commitments plus a Merkle tree over parameters.
- JSON model import, batch inference, and validated inference.
- A prover CLI binary printing the model commitment and output.
- Verification bundle assembly, JSON serialization, and bundle ids.
- Verifier contract: public input parsing, events, and query methods.
- Documentation set, CI with fmt/clippy, and contributor tooling.

[Unreleased]: https://github.com/ZKML-Soroban/ZKML-Soroban/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.0.1
[0.2.0]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.2.0
