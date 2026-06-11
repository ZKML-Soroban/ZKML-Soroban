# Testing Guide

## Run everything

```bash
cargo test --workspace
```

## Layers of tests

- **Unit tests** live next to the code (`#[cfg(test)] mod tests`) and cover the
  numeric core, commitments, inference, and quantization.
- **Integration tests** under `crates/zkml-prover/tests` import bundled example
  models and run them end to end.
- **Contract tests** in `zkml-verifier` use the Soroban test environment to
  exercise `initialize`, `verify_inference`, and the query methods.

## What to test in a PR

Any change to inference, quantization, commitments, or the contract must ship
with tests. Determinism-sensitive code (fixed-point arithmetic) needs explicit
round-trip and edge-case coverage.
