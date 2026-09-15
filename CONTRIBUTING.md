# Contributing to zkml-soroban

Thank you for your interest in contributing to zkml-soroban. This document
provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Pull Request Process](#pull-request-process)
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
- Run `cargo clippy --workspace --all-targets` and do not introduce new warnings.
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
3. Ensure code is formatted (`cargo fmt -- --check`).
4. Ensure you add no new lint warnings (`cargo clippy --workspace --all-targets`).
5. Update documentation if your change affects the public API or
   architecture.
6. Open a pull request with a clear description of the change and its
   motivation.
7. At least one maintainer review is required before merging. Changes to
   `crates/zkml-verifier` or `crates/zkml-common/src/commitment.rs` are
   consensus-critical and get an extra review.

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
