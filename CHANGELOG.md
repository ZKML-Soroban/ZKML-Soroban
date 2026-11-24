# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking
- Verifier contract interface `VERSION` 11. `accept_admin` returns
  `Result<(), VerificationError>` and, without a live proposal, the new
  `PendingAdminMismatch` (18) instead of panicking. `get_risc0_config` and
  `get_risc0_vk` return `Option` instead of panicking before they are set. The
  constructor and `set_model_hash` panic on a model hash that is not 32 bytes
  below the BN254 scalar modulus, which no proof could ever match.

### Security
- Both verification paths refuse a proof point at infinity (Soroban's all-zero
  encoding, which the curve checks accept) with `MalformedProofA`, `B` or `C`.
  An honest Groth16 proof never holds the identity.
- Registering a verifying key (constructor, `set_verification_key`,
  `set_risc0_vk`) pairs its three G2 points, so the host checks they are in the
  prime-order subgroup and a key that would trap every verification is never
  stored. Up to `VERSION` 10 such a key was accepted. Costs 22.7M instructions,
  once per key.
- Admin proposals expire after `ADMIN_PROPOSAL_LEDGERS` (7 days), and the admin
  can withdraw one with `cancel_admin_proposal` (event `("cfg_upd", "adm_cncl")`).
  `get_pending_admin` reads the live proposal.

### Added
- `zkml-prover cycles <MODEL> -i <CSV> --opening <FILE>` executes the guest
  without proving and prints its user cycles, padded cycles and segments in
  seconds (`zkvm` feature); the library function is `prover::count_cycles`.
- CI fails when the reproducible guest's cycles for `credit_lr.json` or
  `kyc_tree.json` grow more than 5% over `crates/zkml-prover/cycles-baseline.json`
  (13,836,264 and 23,268,536 user cycles).
- Criterion benchmarks: `cargo bench -p zkml-common --bench core` (fixed-point
  arithmetic, commitments, inference) and `cargo bench -p zkml-prover --bench
  onnx` (ONNX import). `docs/reference/benchmarks.md` reports measured numbers
  instead of the old estimates.
- `zkml-demo` runs the demo end to end instead of printing a plan: it loads
  the model (`.onnx` or JSON) and runs it natively, proves it (`groth16`
  feature) or takes a pre-built `--bundle`, checks the bundle against the native
  run, deploys the verifier and registers the bundle's guest through the
  `stellar` CLI unless `--contract` names one, submits `verify_receipt`, reads
  `get_result` back and reports stage timings, payload size, the fee charged and
  the roadmap's Phase 1 criteria as pass, fail or not measured. `just demo`
  runs it on testnet. It drops its unused `soroban-sdk` 21 and `tokio`
  dependencies.
- `examples/kyc-demo/train_model.py` also writes `validation_dataset.json`, for
  `zkml-prover validate` to check the quantized tree against scikit-learn, and
  one input row in `features_sample.txt`.
- The CLI reads ONNX models: any subcommand given a `.onnx` file runs it
  through the ONNX importer, so the trained KYC model is proved directly
  instead of being converted to JSON first.

### Changed
- `verify_inference` assembles L with one `g1_msm` instead of four
  multiplications and four additions: 28,956,777 CPU instructions on the WASM,
  down from 31,006,558. `verify_receipt` is 29,578,818. The WASM budget tests
  fail above 30.5M, and a new one measures `set_risc0_vk`.
- The verifier's tests moved out of `lib.rs` (from 3,988 lines to 1,281) into
  `crates/zkml-verifier/src/tests/`, one file per module. Module names are kept,
  so test paths and `test_snapshots/` do not move. The contract WASM is byte for
  byte the same.
- A decision tree's `class_label` is its leaf value rounded down, instead of
  always `0`. A classifier tree's leaf holds the predicted class (the ONNX
  importer and `tools/sklearn_to_json.py` write it that way), so the KYC risk
  tier now arrives in `class_label` as well as in `output`. The guest compiles
  this, so the reproducible image id changes to
  `fcf4b6b101f93c6bb7b6b89e778c494d113e55c0e6a34129de6b30257617f557`, and the
  golden receipt and RISC Zero parameters in `crates/zkml-verifier/testdata/`
  are regenerated from it.

### Fixed
- Verifier tests that could not fail now test what their names say:
  `verify_distinct_inputs_independent` and `verify_distinct_outputs_independent`
  sent 72-byte inputs that never reached the nullifier, `verify_multi_scalar_output`
  only checked that one error did not occur, `set_model_hash_unauthorized_fails`
  never called `set_model_hash`, and `model_commitment_is_reproducible` compared
  a value with itself. They now use real proofs, and a new cross-check runs
  commitments from `zkml-common` through the constructor and `verify_inference`.
  Each was checked to fail against a broken contract (no nullifier check, no
  admin auth, big-endian public inputs).
- New `test_exact_auth` covers every admin entry point with `mock_auths`
  instead of `mock_all_auths`: no authorization, the wrong signer and the
  right signer. Removed five orphaned snapshots under `test_snapshots/test_verify/`.

## [0.0.5] - 2026-11-24

Security release. RISC Zero 3.0.x Groth16 receipts can be forged
(GHSA-g6qg-9j8p-7j87 upstream), so a verifier registered with the parameters
`0.0.4` produced accepts made-up results. The prover moves to RISC Zero 3.1.0,
and a deployed verifier has to register the new parameters. Tracked as
GHSA-hmx6-94gg-9r4q. See Security and Breaking.

### Security
- RISC Zero 3.1.0: `risc0-zkvm` 3.1.0, `risc0-build` 3.0.7, `risc0-groth16`
  3.0.6 and the 4.1.0 recursion and rv32im circuits, regenerated with a new
  Groth16 trusted setup. In 3.0.x the recursion circuit and the Groth16 wrapper
  under-constrain the claim digest's bit decompositions, so anyone holding one
  valid Groth16 receipt can have an arbitrary claim, image id and journal
  included, accepted (RISC Zero GHSA-g6qg-9j8p-7j87 and GHSA-mm6c-35cm-g4x2,
  critical). `verify_receipt` itself is unchanged and correct; what it trusts is
  the registered parameters, and `export-vk` now emits the 3.1.0 ones: selector
  `e71e6620`, control root
  `e6feef13db7aed4a731c7b0852de1f5c89b137726073f625d05333184d3a551d`, BN254
  control id `de81d091ba14062acb5023b3b36deee95624e3fd47475cb7ff64aed5f9eac20a`
  and the new verifying key. A deployed verifier should be paused, given these
  with `set_risc0_config` and `set_risc0_vk`, and unpaused. The two testnet
  verifiers listed in the deployment guide were paused on 2026-11-24.
- The workspace lockfile moves `url` to 2.5.9, which fixes RUSTSEC-2026-0592
  (stack overflow in `Url::origin()` on nested `blob:` URLs). `url` reaches
  `zkml-prover` through `reqwest`, which RISC Zero's `bonsai-sdk` pulls in; the
  prover never calls `origin()`, and neither the contract nor the zkVM guest
  links `url`.

### Breaking
- Receipts from `0.0.4` and earlier do not verify against the 3.1.0
  parameters; prove again with `0.0.5`. The reproducible guest's image id
  changes to `2d651704f44c82eabe53dcc17ebe537f2f760cf21db11ec0c16859ccd3d2e181`, and the golden receipt and RISC Zero parameters in
  `crates/zkml-verifier/testdata/` are regenerated from it.
- Proving needs the RISC Zero 3.1.0 toolchain (`rzup install cargo-risczero
  3.1.0`, `rzup install r0vm 3.1.0`), and local Groth16 compression pulls
  `risczero/risc0-groth16-prover:v2026-10-20.1`.

### Changed
- The recursion circuits now ship inside crates
  (`risc0-circuit-recursion-zkrs`), so builds no longer download them from
  RISC Zero's S3 bucket, which was retired on 2026-11-23 and answers 403. A
  clean build with the `prove` feature failed until this upgrade. `zip` 2.3
  leaves the tree, and the audit drops its six ignores.

## [0.0.4] - 2026-11-20

Security release. Model and input commitments become hiding, fixing
GHSA-x4pc-c7xq-p474: every model has to be registered again under a private
opening, and the guest image id changes. See Breaking and Security.

### Breaking
- Model and input commitments are hiding. `commit_model(model, opening)` and
  `commit_inputs(features, opening)` in `zkml-common` take an `Opening`, 32
  secret random bytes chosen when a model is registered, and start the Poseidon
  chain from `Poseidon(domain, opening)`, with new domain tags `3` and `4`. The
  zkVM guest reads the opening as a private input and never writes it to the
  journal. Every model commitment changes, so models have to be registered
  again, and the guest image id changes to
  `49a1c07fd245cf761ea47f64cd36634b70b41cc3aff5c73da4d0dbb7779fdc32`: a verifier
  needs `set_risc0_config` with it. The golden receipt in
  `crates/zkml-verifier/testdata/` is regenerated from that guest, under a
  fixed public test opening.
- `zkml-prover commit` and `prove` need `--opening FILE` (64 hex characters;
  all-zero is refused), and `infer` and `inspect` no longer print a commitment.
  In the prover library, `model_commitment`, `input_commitment`, `journal_for`,
  `generate_proof`, `generate_receipt` and `prove_groth16` take the opening.
- `examples/kyc-demo/deploy.sh` takes `--opening`, and otherwise generates one
  in `target/kyc-demo/model.opening`.

### Security
- Fixes GHSA-x4pc-c7xq-p474 (medium). Model and input commitments were
  deterministic hashes, so the parameters of a model with few or predictable
  values, or a predictable input, could be recovered from the public commitment
  by hashing candidates until one matched. With the opening mixed in first, a
  commitment reveals nothing to anyone who does not hold the opening and still
  binds a proof to one model. Commitments made with `0.0.3` or earlier stay
  exposed: register models again, and replace a model whose parameters are
  sensitive. `*.opening` files are ignored by git. Reported privately through
  GitHub's vulnerability reporting.

## [0.0.3] - 2026-11-20

Third crates.io release (`zkml-common`, `zkml-verifier`), and the first whose
contract has verified a real proof on Stellar testnet. The contract interface
moves from `VERSION` 9 to 10, the guest image id changes, ONNX tree imports
change their leaf values, and `zkml-prover prove` needs `--groth16` or
`--no-proof`. See Breaking.

### Breaking
- ONNX tree imports change value at every leaf that carries one weight per
  class, as skl2onnx writes them: the leaf now holds the predicted class (see
  Fixed), so the imported model, and its commitment, differ from `0.0.2`.
- The contract `VERSION` goes from `9` to `10`: the constructor's native
  verification key is now `Option<VerificationKey>`. A verifier that only checks
  RISC Zero receipts is deployed without one, and `verify_inference` then
  returns the new error `NativeKeyNotConfigured` (16). With the `stellar` CLI,
  leave out `--vk`; from Rust, pass `Some(vk)` or `None`.
- `VERSION` 10 also changes three error codes a client may match on. A paused
  contract returns the new `ContractPaused` (17) from both verification entry
  points instead of `VerificationFailed`. `verify_inference` reports a
  wrong-length `proof_c` as `MalformedProofC` instead of `MalformedProofA`. And a
  journal whose reserved bytes (7 and 88 to 95) are not zero is refused with
  `MalformedJournal`.
- `zkml-prover prove` needs `--groth16` or `--no-proof`. Without a flag it
  used to write the legacy v1 bundle, which carries no proof, so a script that
  forgot `--groth16` got a bundle that looked usable. It now exits with code 2
  and names both flags. `--no-proof` writes the same legacy bundle as before,
  and its warning goes to stderr, so stdout is the JSON alone.
- `JournalV1::decode` in `zkml-common` refuses a non-zero reserved byte with
  the new `JournalError::NonZeroReserved`, so a `match` on `JournalError` needs
  an arm for it. The guest has always written zeros there, so no real journal
  changes.

### Added
- The verifier runs on Stellar testnet. On 2026-11-17 a `VERSION` 10 contract
  verified the golden receipt, a real Groth16 proof from the reproducible
  guest, in transaction `06e534c2...`. On 2026-11-20 the KYC demo ran end to
  end: the KYC tree, proved with the `0.0.3` guest, verified on a second
  contract in transaction `f96d7203...`. The contract ids and every transaction
  are listed in the deployment guide, under "Live on testnet".
- `examples/kyc-demo/deploy.sh` is rewritten for the `stellar` CLI and runs the
  KYC demo end to end: it creates and funds an identity, deploys with the
  constructor and no native key, registers the reproducible guest, proves with
  Groth16, submits the receipt with `verify_receipt` and reads the result.
  `--bundle` and `--risc0-params` take files made elsewhere, for machines that
  cannot prove. The old script called the `soroban` CLI and `initialize`, and
  passed all-zero placeholder keys.
- `ZKML_GUEST_DOCKER=1` builds the zkVM guest inside RISC Zero's builder image
  (pinned to `r0.1.91.1`, since `risc0-build`'s default image ships a Rust older
  than the workspace MSRV), so the guest image id is the same on every machine.
  The default local build is unchanged; `export-vk` warns when its image id is
  machine-specific. CI builds the reproducible guest and checks that the
  committed golden receipt was proved by it.
- Contract tests for the instance TTL policy: the constructor, both
  verification entry points on success, `set_risc0_config`, `set_risc0_vk` and
  both steps of the admin transfer renew the TTL; a failed verification and the
  other admin setters do not. The original TTL tests had been lost in a merge.

### Changed
- Documentation: the roadmap is reviewed for November 2026. The reproducible
  guest build and the testnet deployment are done; Phase 1 has remote proving,
  ONNX models in the CLI with the demo runner, and the commitment sponge left.
  Pages and code comments that
  still told readers to call `initialize` describe the constructor. The known
  limitations, the model serialization in the commitments page, the KYC demo
  status, the order of the `verify_inference` checks and the post-quantum
  migration table describe the current code.
- The golden receipt and RISC Zero parameters in
  `crates/zkml-verifier/testdata/` are regenerated from the reproducible guest
  build: after the reserved-byte check in `JournalV1::decode` (see Breaking),
  which moved lines in a file the guest compiles, and again for this release,
  since the guest compiles `zkml-common` and its version is part of the build.
  The guest image id of `0.0.3` is
  `618662ee2f9615d58b6ac5bacc874da9a1c3f7aec93109a30a820d892744e085`; a
  verifier configured with an earlier id needs `set_risc0_config` again.

### Removed
- 27 contract test snapshots whose tests no longer exist, most of them from
  before the constructor replaced `initialize`.

### Fixed
- ONNX `TreeEnsembleClassifier` import kept the last class weight it read for
  each leaf, so a skl2onnx tree imported every leaf as the probability of its
  last class: in the three-class iris fixture, the leaf that predicts class 1
  with probability 0.907 imported as `0.093`. A leaf with one weight per class
  now holds its predicted class, mapped through `classlabels_int64s`; ties go to
  the lowest class id. Text labels are refused, and class attribute lists of different
  lengths are an error instead of an out-of-bounds index.
- The `wasm_budget` assertion messages printed a run of spaces in the middle of
  the sentence.
- `verify_inference` validates its three proof points before the pairing, as
  `verify_receipt` already did, so a corrupted point returns `MalformedProofA`,
  `MalformedProofB` or `MalformedProofC` instead of trapping in the host. The
  check runs after the replay check, so a replay still costs no pairing.
  Measured on the WASM, `verify_inference` now costs 31,006,558 CPU
  instructions, up from 30,208,647, still under the 32M ceiling CI enforces.

### Security
- The zkVM guest lockfile moves `ruint` to 1.20.1, which fixes
  RUSTSEC-2026-0220, and the dependency audit no longer ignores it for the
  guest.
- The workspace lockfile moves `quinn-udp` to 0.5.16, which fixes
  RUSTSEC-2026-0538. The audit ignores three new advisories in `zip` 2.3
  (RUSTSEC-2026-0429, 0545 and 0548), which RISC Zero 3.0.6 pins and only uses
  to read the recursion circuits embedded in its own crate; the fix is in
  `zip` 9, a major version RISC Zero has not adopted.
- The constructor and `set_verification_key` refuse a native key without five
  `ic` points, or with a point off its curve or at infinity. A receipt-only
  deployment previously had to pass some native key, and the placeholder the
  tests and deploy script used, with beta, gamma and delta at infinity, accepted
  an all-zero proof for any public input, so anyone could record a verified
  result. No contract had been deployed with it.

## [0.0.2] - 2026-10-25

Second crates.io release (`zkml-common`, `zkml-verifier`). The contract
interface moves from `VERSION` 5 to 9, and model commitments change, so a
model registered with `0.0.1` has to be registered again. See Breaking.

### Breaking
- The contract `VERSION` goes from `5` to `9` in this release:
  - `5 -> 6`: `verify_receipt` and the RISC Zero configuration calls below.
  - `6 -> 7`: `initialize` is gone. The admin, model commitment and
    verification key are passed to a `__constructor` at deploy, so nobody can
    initialize a deployed contract before its owner does. `set_admin` is
    replaced by the two-step `propose_admin` / `accept_admin`. New error
    `NonCanonicalPublicInput` (15).
  - `7 -> 8`: `get_result` and `get_model_hash` return `Option` instead of
    panicking when nothing is stored.
  - `8 -> 9`: `claim_digest` returns `MalformedJournal` for a journal that is
    not 96 bytes instead of panicking.
- Model commitments change. The element stream behind the Poseidon commitment
  now starts with the model kind and length-prefixes every variable-length part,
  so two models of different shape can no longer share a commitment. A model
  hash computed with `0.0.1` does not match the one this release computes for
  the same model; re-register it with `set_model_hash`.
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
  with the contract. It returns `MalformedJournal` for a journal that is not 96
  bytes.
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
- `FixedPoint::try_quantize`, which refuses `NaN`, infinities and values that do
  not fit in `i64` instead of saturating.
- `tools/sklearn_to_json.py` converts a scikit-learn model to the JSON exchange
  format, and `tools/inspect_bundle.py` prints what the contract receives from a
  bundle and rebuilds its 80-byte public input (`--check` sets the exit code).
- Golden vectors for logistic regression edge cases, and every success case now
  asserts its `class_label` as well as its score.
- Contract tests for the pause path and the verification counter.
- CI jobs: a dependency audit of both lockfiles (`cargo audit`), a build on the
  declared MSRV (Rust 1.91), the `wasm_budget` tests on the compiled contract
  with a 32M-instruction ceiling, and clippy with `--features groth16`. Clippy
  with `-D warnings` is now blocking.
- `tests/verifier_fixtures.rs` checks the RISC Zero parameters in the verifier's
  fixtures against the pinned RISC Zero version.

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
- The contract's events are declared with `#[contractevent]`, so their schemas
  are part of the contract spec. Their topics and data are unchanged.
- Every admin setter publishes a `cfg_upd` event, including
  `set_verification_key`, `set_model_hash` and `set_pause`, since `log!` is
  compiled out of the deployed contract.
- `commit` and `prove` run the model range and overflow-bounds checks by default
  and refuse a model that fails them (`--skip-safety-check` opts out), and the
  ONNX importer refuses non-finite or out-of-range parameters.
- `verify_inference` checks the nullifier before the pairing, so a replay is
  refused without paying for it, as `verify_receipt` already did.
- The zkVM guest uses the fallible inference path, so a malformed model fails
  with a typed reason instead of a panic.
- Documentation: the verifier README, TTL policy, error table, fixed-point
  rounding rules and quickstart describe the current contract, and
  `CONTRIBUTING.md` explains what a change to the guest costs. The contributing
  docs page moved to `project/development`, with a redirect.

### Removed
- `OnnxImportError::ExtractionNotImplemented`, which nothing constructed once
  tree and linear extraction landed.

### Fixed
- The BN254 control id is byte-reversed before being used as a public input, as
  `risc0_groth16::Verifier::new` does. Without it every pairing check would
  fail.
- `tagged_struct` refuses more digests or data words than its fixed buffer holds,
  instead of silently hashing a truncated struct in release builds.
- `cargo test --workspace` passes on Windows: the main thread gets the same 8 MB
  of stack it has on Linux and macOS.
- The ONNX protobuf wire types match the ONNX schema, so files written by real
  exporters decode; real skl2onnx and `onnx` fixtures guard it.
- The KYC demo model is exported with opset 17, which the importer accepts.
- The CLI accepts a feature vector whose first value is negative
  (`-i -0.5,0.2`).
- The golden receipt was regenerated for the current guest, and the
  `wasm_budget` tests, broken since the constructor change, run again.
- `compute_expected.py` rounds halfway cases away from zero, as
  `FixedPoint::quantize` does, and a flaky fixed-point property test no longer
  aborts on too many rejected inputs.

### Security
- The contract accepts only the canonical encoding of each public input. A
  `model_hash` or `input_hash` at or above the BN254 modulus is refused, so one
  proof can no longer be recorded again under `x + r` with a fresh nullifier.
- The model commitment change under Breaking closes a collision between models
  of different shape, which let a model of another kind pass `verify_receipt`
  under a registered hash.
- Dependency updates for published advisories: `ruint` 1.20.1
  (RUSTSEC-2026-0220), `rustls` 0.23.46 (RUSTSEC-2026-0285, -0346, -0348, -0349),
  `quinn-proto` 0.11.19 (RUSTSEC-2026-0338 to -0342), `rustls-webpki` 0.103.16
  (RUSTSEC-2026-0382) and `webpki-roots` 1.0.10 (RUSTSEC-2026-0381). They come in
  through the RISC Zero host stack; none reaches the published crates. The
  zkVM guest lockfile still pins `ruint` 1.19.0, because updating it changes the
  guest's image id.

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

[Unreleased]: https://github.com/ZKML-Soroban/ZKML-Soroban/compare/v0.0.5...HEAD
[0.0.5]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.0.5
[0.0.4]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.0.4
[0.0.3]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.0.3
[0.0.2]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.0.2
[0.0.1]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.0.1
[0.2.0]: https://github.com/ZKML-Soroban/ZKML-Soroban/releases/tag/v0.2.0
