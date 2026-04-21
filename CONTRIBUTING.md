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

This project follows the [Contributor Covenant](https://www.contributor-covenant.org/)
code of conduct. By participating, you are expected to uphold this standard.

## Development Environment

### Prerequisites

- **Rust** (stable, 1.79 or later): [Install via rustup](https://rustup.rs/)
- **Stellar CLI**: Required for contract compilation and deployment.
  ```bash
  cargo install --locked stellar-cli@26.0.0
  ```
- **wasm32-unknown-unknown target**: Required for compiling Soroban contracts.
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- **RISC Zero toolchain** (Phase 1 prover only):
  See [RISC Zero installation guide](https://dev.risczero.com/api/zkvm/install).

### Building

```bash
# Build all workspace crates
cargo build

# Build the verifier contract for deployment
cargo build --release --target wasm32-unknown-unknown -p zkml-verifier

# Run all tests
cargo test --workspace
```

## Project Structure

```
crates/
  zkml-common/     Shared types (models, fixed-point, proof structures)
  zkml-prover/     Off-chain inference and proof generation
  zkml-verifier/   On-chain Soroban verification contract
docs/              Technical documentation
```

Refer to [docs/architecture.md](docs/architecture.md) for a detailed
breakdown of each component.

## Coding Standards

- Follow standard Rust formatting: run `cargo fmt` before committing.
- All public items must have documentation comments (`///` or `//!`).
- Run `cargo clippy --workspace` and resolve all warnings.
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
4. Ensure no lint warnings (`cargo clippy --workspace`).
5. Update documentation if your change affects the public API or
   architecture.
6. Open a pull request with a clear description of the change and its
   motivation.
7. At least one maintainer review is required before merging.

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
