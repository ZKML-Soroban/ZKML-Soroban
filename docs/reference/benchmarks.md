---
title: "Benchmarks"
description: "Native inference baselines and the measured on-chain verification budget."
icon: "gauge"
---

## On-chain verification budget

Measured with the Soroban SDK cost estimator (`env.cost_estimate().budget()`) in
`test_verifier_accept_path_and_resource_budget`, using a fixture that genuinely
satisfies the pairing equation:

| Operation                                   | CPU instructions | Memory (bytes) | Regression threshold (CPU / memory) |
| ------------------------------------------- | ---------------- | -------------- | ----------------------------------- |
| Full `verify_inference` (L assembly + pairing) | 29,289,569    | 277,964        | 50,000,000 / 10,000,000             |

Breakdown:

- **L assembly:** 4 G1 scalar multiplications and 4 G1 additions for
  `L = IC[0] + sum(x_i * IC[i + 1])`.
- **Pairing check:** one 4-pair BN254 pairing via the CAP-0074 host function.
- **Fixture:** non-degenerate G2 points from the soroban-env-host test suite,
  G1 generator for every IC entry, and `C = -L` so the product of pairings is 1.

The test fails if the cost exceeds the threshold, which catches large
regressions.

Run it with output:

```bash
cargo test -p zkml-verifier test_verifier_accept_path_and_resource_budget -- --nocapture
```

## Native inference

Sanity baselines measured with the `timing` feature on a developer laptop:

| Model               | Inputs | Native inference |
| ------------------- | ------ | ---------------- |
| Logistic regression | 4      | Under 1 µs       |
| Decision tree       | 3      | Under 1 µs       |
| Tiny MLP (8-8-1)    | 8      | A few µs         |

## Proving

Measured with `crates/zkml-prover/tests/groth16_bundle.rs::real_groth16_bundle_verifies`
on `examples/models/credit_lr.json` (logistic regression, 4 features):

| Step | Value |
| ---- | ----- |
| Machine | 24 threads, 15 GB RAM, Arch Linux x86_64 under WSL2 |
| Peak prover memory | about 8.9 GB (`r0vm`) |
| Total wall clock, prove + compress | 1,506 s |
| Seal | 260 bytes (4-byte selector + 256-byte proof) |
| Journal | 96 bytes |
| Total on-chain payload | 356 bytes |

Reproduce it with:

```bash
RISC0_DEV_MODE=0 cargo test -p zkml-prover --features groth16 \
  --test groth16_bundle real_groth16 -- --ignored --nocapture
```

Two things this table is not: a floor and a ceiling. The model is tiny, so
almost all of that time is the fixed cost of the zkVM and the recursion, not the
inference; a decision tree or a small MLP lands in the same range. And a machine
with more memory and a GPU-enabled prover is much faster. Treat 25 minutes as
what a laptop does, not as what the design costs.

The 260-byte seal meets the Phase 1 target of a proof under 500 bytes.

## Not yet measured

- Proving time per model family (tree, MLP) under real mode
- Remote proving latency, once a Boundless client exists
- End-to-end latency on testnet, which needs on-chain verification (issue #84)
