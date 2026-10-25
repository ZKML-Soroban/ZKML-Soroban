# Contributing to zkml-soroban

Thank you for your interest in contributing to zkml-soroban. This document
provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Pull Request Process](#pull-request-process)
- [Changing the zkVM guest](#changing-the-zkvm-guest)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md), adapted from the
Contributor Covenant 2.1. By participating, you are expected to uphold it. Report
unacceptable behavior as described in its Enforcement section.

## Development Environment

### Prerequisites

- **Rust** (stable, 1.91 or later): [Install via rustup](https://rustup.rs/).
  `rust-toolchain.toml` pins the channel, components and the `wasm32v1-none` target.
- **Stellar CLI** (deployment only):
  [installation guide](https://developers.stellar.org/docs/tools/cli).
- **RISC Zero toolchain 3.0.6** (zkVM proving only):
  [installation guide](https://dev.risczero.com/api/zkvm/install).
- **just** (optional task runner): [casey/just](https://github.com/casey/just).
- **Node.js 22** (optional, documentation preview only).

### Building

```bash
# Build all workspace crates
cargo build

# Build the verifier contract for deployment
cargo build -p zkml-verifier --target wasm32v1-none --profile contract

# Run all tests
cargo test --workspace

# zkVM guest tests (requires the RISC Zero toolchain)
RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm

# Preview the documentation site
cd docs && npx mint dev
```

## Project Structure

```
crates/
  zkml-common/     Shared deterministic core (models, fixed point, inference, commitments)
  zkml-prover/     Off-chain ONNX import, quantization, CLI and zkVM proving
  zkml-verifier/   On-chain Soroban verification contract
  zkml-demo/       End-to-end demo runner
methods/           RISC Zero guest program
examples/          Example models and the KYC demo
docs/              Mintlify documentation site
```

Refer to [docs/concepts/architecture.md](docs/concepts/architecture.md) for a
detailed breakdown of each component.

### Documentation

Pages live under `docs/` and are listed in `docs/docs.json`. Every page needs
`title` and `description` frontmatter, and internal links are root-relative
without an extension (for example `/concepts/architecture`). Adding a page
means adding it to the navigation in `docs/docs.json`.

## Coding Standards

- Follow standard Rust formatting: run `cargo fmt` before committing.
- All public items must have documentation comments (`///` or `//!`).
- Run `cargo clippy --workspace --all-targets -- -D warnings`. CI fails on any
  warning, so the workspace has to stay at zero.
- Unsafe code is not permitted without explicit justification in comments.
- Maintain test coverage for all new inference logic and quantization
  utilities.

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
feat(prover): add MLP inference with quantized ReLU
fix(verifier): correct public input encoding for BN254
docs: update architecture diagram with Phase 2 circuits
test(common): add round-trip tests for fixed-point edge cases
```

## Pull Request Process

1. Fork the repository and create a feature branch from `main`.
2. Ensure all tests pass (`cargo test --workspace`).
3. Ensure code is formatted (`cargo fmt --all --check`).
4. Ensure clippy reports no warnings (`cargo clippy --workspace --all-targets -- -D warnings`).
   `just check` runs it together with the format check.
5. Update documentation if your change affects the public API or
   architecture.
6. Open a pull request with a clear description of the change and its
   motivation.
7. At least one maintainer review is required before merging. Changes to
   `crates/zkml-verifier` or `crates/zkml-common/src/commitment.rs` are
   consensus-critical and get an extra review.

## Changing the zkVM guest

The guest's **image id** is the hash of the guest binary. The contract only
accepts receipts from the image id its admin registered, so changing it is
never a local change. It changes when any of these change:

- anything under `methods/guest/`, including `Cargo.toml` and `Cargo.lock`
  (a dependency bump there counts);
- any code in `zkml-common` that the guest compiles in;
- the RISC Zero version, pinned with `=` in the root `Cargo.toml`,
  `methods/Cargo.toml` and `methods/guest/Cargo.toml`.

It also depends on the machine that builds it. `methods/build.rs` calls
`risc0_build::embed_methods()`, which compiles the guest with the host's
toolchain, so the same commit can produce different binaries: the CI runners
and a local Linux machine get different image ids. RISC Zero's Docker build
(`GuestOptions::use_docker` in `risc0-build`) is what makes the id
reproducible; the project does not use it yet. Until it does, register the
image id that `export-vk` prints on the machine that produces the proofs.

A change to the guest, to `zkml-common` or to RISC Zero costs three things,
so keep it in its own pull request:

1. **The golden fixtures go stale.** `crates/zkml-verifier/testdata/` holds a
   real receipt and the RISC Zero parameters it verifies against. The contract
   tests read the image id from the receipt itself, so they do not notice. The
   `verifier_fixtures` test in `zkml-prover` (run by the zkVM CI jobs) does
   catch a RISC Zero bump, because it compares the parameters with what the
   pinned version derives. Regenerating takes about 22 minutes of CPU and
   needs x86_64 Linux with Docker:

   ```bash
   RISC0_DEV_MODE=0 cargo test -p zkml-prover --features groth16 \
       --test groth16_bundle real_groth16 -- --ignored --nocapture
   cp target/groth16_bundle.json crates/zkml-verifier/testdata/groth16_bundle.json
   cargo run -p zkml-prover --features zkvm -- export-vk --format json \
       -o crates/zkml-verifier/testdata/risc0_vk.json
   ```

   Run both commands on the same machine, then `cargo test -p zkml-verifier`,
   and commit the fixtures and any updated snapshots with the change.
2. **Deployed verifiers stop accepting new proofs.** A contract keeps the image
   id it was configured with, so every receipt from the new guest fails with
   `VerificationFailed` until its admin calls `set_risc0_config` with the
   values `export-vk --format soroban-args` prints.
3. **The model commitment may change too.** If the change touches
   `zkml-common/src/commitment.rs`, the model hash changes as well, and
   `set_model_hash` has to be called with the new value. That file is
   consensus-critical and gets an extra review.

## Releases

Maintainers release by bumping `workspace.package.version` in the root
`Cargo.toml` and moving the `Unreleased` section of `CHANGELOG.md` under the
new version. Merging to `main` publishes `zkml-common` and `zkml-verifier` to
crates.io and creates the `vX.Y.Z` GitHub Release.

## Reporting Issues

Open an issue on GitHub with the following information:

- **Summary**: A clear, concise description of the problem.
- **Environment**: Rust version, OS, Stellar CLI version.
- **Steps to reproduce**: Minimal sequence of actions to trigger the issue.
- **Expected behavior**: What you expected to happen.
- **Actual behavior**: What actually happened, including error messages or
  logs.

For security vulnerabilities, please refer to [SECURITY.md](SECURITY.md)
instead of opening a public issue.
