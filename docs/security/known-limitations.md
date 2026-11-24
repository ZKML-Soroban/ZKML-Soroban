---
title: "Known limitations"
description: "Honest list of what does not work yet and design gaps under review."
icon: "circle-exclamation"
---

zkml-soroban is pre-production. This page lists current gaps so integrators can
plan around them. Each item is tracked on the [roadmap](/project/roadmap).

## Proving

- **Testnet only.** A real receipt has been verified on Stellar testnet (see
  [Live on testnet](/guides/deployment#live-on-testnet)), at about 30 million
  CPU instructions against a 100 million limit. Nothing has been deployed to
  mainnet, and the contract has not had a third-party audit.
- **A crafted G2 point in a proof can still trap.** Both entry points validate
  their proof points and return a typed error for a corrupted one or one at
  infinity, but a proof's B on the curve and outside the subgroup traps in the
  host. Corruption does not produce one; it takes a deliberately crafted point,
  and the transaction aborts without recording anything. Keys are checked for
  this at registration, since `VERSION` 11.
- **Groth16 compression needs x86_64 Linux with Docker.** The Circom witness
  generator image is not published for other platforms (risc0 issue #1749), so
  `prove_groth16` fails fast with `UnsupportedPlatform` elsewhere. The `cuda`
  feature wraps natively on Linux with an NVIDIA GPU in theory, but the native
  wrap crashed on the one card it was tried on; see
  [Proving](/concepts/proving).
- **A local guest build has its own image id.** By default the guest is
  compiled with the local RISC Zero toolchain, so its image id differs between
  machines, and a contract only accepts receipts from the id its admin
  registered. Build with `ZKML_GUEST_DOCKER=1` (RISC Zero's pinned builder
  image) to get the reproducible id, and prove with that same build.
- **No remote proving.** RISC Zero shut down Bonsai in December 2025.
  `--backend boundless` returns `ProveError::RemoteBackend` until a Boundless or
  self-hosted Bento client is written.
- **Dev mode produces no proof.** `RISC0_DEV_MODE=1` makes receipts fake, so
  `prove_groth16` refuses to run under it rather than emitting a bundle that
  looks real.
- **The legacy `prove --no-proof` output carries no proof.** It stays for
  inspection and prints a warning; it must never be accepted as evidence.
  `prove` refuses to run without `--groth16` or `--no-proof`, so it is never
  written by default.
- **Panicking decision path.** `run_inference_with_decision` can panic (MLP
  overflow, tree iteration limit). The fallible
  `try_run_inference_with_decision` is what the proving path uses, but the
  panicking function is still public.

## Commitments

- Commitments made with `0.0.3` or earlier are deterministic hashes. For a
  model with few or predictable parameters, or a predictable input, the value
  can be recovered from such a commitment by trying candidates, and a
  commitment already published stays exposed. Register the model again under
  an opening with `0.0.4`, and replace it too if its parameters are sensitive.
- One opening covers a model and all its inputs, so an observer can tell that
  two proofs used the same input under the same model, without learning the
  input. That is also what keeps one inference from being recorded twice.
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
- **A verifier keeps the RISC Zero parameters it was given.** The contract
  does not know which RISC Zero version a key belongs to. One registered with
  the 3.0.x parameters, which `0.0.4` and earlier printed, accepts forged
  receipts (RISC Zero GHSA-g6qg-9j8p-7j87) until its admin registers the 3.1.0
  parameters from `0.0.5`.
- **`get_admin` can panic.** Only on a contract without stored state, which the
  constructor rules out. Since `VERSION` 11 the other getters return `Option`.
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
