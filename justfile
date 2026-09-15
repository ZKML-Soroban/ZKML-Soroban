# Developer tasks. Run `just <task>` (https://github.com/casey/just).

default:
    @just --list

# Build the whole workspace.
build:
    cargo build --workspace

# Run all tests.
test:
    cargo test --workspace

# Format check and lints.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets

# Build the Soroban verifier WASM (target/wasm32v1-none/contract/zkml_verifier.wasm).
contract:
    cargo build -p zkml-verifier --target wasm32v1-none --profile contract

# Run zkVM guest tests in dev mode (requires the rzup toolchain).
zkvm:
    RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm

# Dry-run packaging of the publishable crates.
package:
    cargo package -p zkml-common
    cargo package -p zkml-verifier

# Preview the documentation site (requires Node.js).
docs:
    cd docs && npx mint dev
