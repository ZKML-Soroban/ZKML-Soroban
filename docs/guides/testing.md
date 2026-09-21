---
title: "Testing"
description: "Test layers, how to run them, and what a pull request must cover."
icon: "vial"
---

## Run everything

```bash
cargo test --workspace
```

The zkVM suites need the RISC Zero toolchain and run in dev mode:

```bash
RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm
```

Check that `zkml-common` still builds without `std`:

```bash
cargo build -p zkml-common --no-default-features
```

## Test layers

| Layer                 | Location                                        | Covers                                              |
| --------------------- | ----------------------------------------------- | --------------------------------------------------- |
| Unit tests            | `#[cfg(test)]` modules next to the code          | Fixed-point core, activations, commitments, inference, quantization |
| Property tests        | `crates/zkml-common/tests/*_props.rs`            | Fixed-point algebra, inference determinism, ReLU monotonicity, argmax stability, no panics |
| Snapshot tests        | `insta` snapshots in `crates/zkml-common/src/snapshots/` | Commitment and Merkle digests              |
| Golden vectors        | `crates/zkml-prover/tests/golden_vectors.rs`, `tests/vectors/*.json` | Expected outputs and structural errors |
| ONNX import           | `tests/onnx_import.rs`, `tests/onnx_tree_extraction.rs`, `tests/tinymlp_inference.rs` | Extraction and rejection paths |
| CLI                   | `crates/zkml-prover/tests/cli.rs`                | Subcommands, input parsing, exit codes              |
| zkVM                  | `tests/zkvm_receipt.rs`, `tests/zkvm_differential.rs` (feature `zkvm`) | Receipt journals vs native inference |
| Contract              | `crates/zkml-verifier/src/lib.rs` test modules   | Initialize, guards, admin auth, events, accept path and resource budget (the TTL tests were lost in a merge; their snapshots remain in `test_snapshots/test_ttl/`) |

Contract tests write snapshots to `crates/zkml-verifier/test_snapshots/`; commit
updated snapshots together with the change that produced them.

## Regenerating golden vectors

`crates/zkml-prover/tests/vectors/compute_expected.py` recomputes expected
outputs. See `tests/vectors/README.md`.

## CI

GitHub Actions (`.github/workflows/ci.yml`) runs on every push and pull request
to `main`:

| Job                 | What it checks                                                |
| ------------------- | ------------------------------------------------------------- |
| Format              | `cargo fmt --all --check`                                     |
| Clippy              | `cargo clippy --workspace --all-targets` (advisory for now)   |
| Test                | `cargo test --workspace`                                      |
| zkml-common without std feature | `cargo build -p zkml-common --no-default-features`            |
| Verifier WASM       | `cargo build -p zkml-verifier --target wasm32v1-none --profile contract` |
| zkVM guest (dev mode) | zkVM tests in dev mode, the RISC Zero digest cross-checks, and the `groth16` surface |
| Groth16 compression (manual) | Real proving and verification; run from the Actions tab |
| Docs (Mintlify)     | `mint broken-links` over `docs/`                              |

## What a pull request must include

- Tests for any change to inference, quantization, commitments, or the contract.
- Round-trip and edge-case coverage for determinism-sensitive arithmetic.
- Updated snapshots when a digest intentionally changes, called out in the PR.
