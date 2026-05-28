.PHONY: build test check contract

build:
	cargo build --workspace

test:
	cargo test --workspace

check:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

contract:
	cargo build --release --target wasm32-unknown-unknown -p zkml-verifier
