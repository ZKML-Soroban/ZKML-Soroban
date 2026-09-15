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

## Not yet measured

- zkVM proving time (real mode) per model family
- Groth16 wrap time (local Docker and a remote prover)
- End-to-end latency on testnet

These depend on the pending Groth16 compression work.
