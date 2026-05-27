# Common developer tasks. Run `just <task>`.

# Build the whole workspace.
build:
    cargo build --workspace

# Run all tests.
test:
    cargo test --workspace

# Format and lint.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# Build the verifier contract for deployment.
contract:
    cargo build --release --target wasm32-unknown-unknown -p zkml-verifier
