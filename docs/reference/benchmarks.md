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
| Full `verify_receipt` (claim digest + L assembly + pairing) | 30,677,367 | 326,021 | 60,000,000 / 12,000,000 |

Verifying a RISC Zero receipt costs 4.7% more than the native-circuit path and
uses 31% of the 100 million instructions a Soroban transaction is allowed.
Reconstructing the claim digest is eight SHA-256 host calls and one extra scalar
multiplication, which is cheap next to the pairing that dominates both numbers.
The receipt figure is measured on the golden fixture in
`crates/zkml-verifier/testdata/`, so it is the cost of a verification that
succeeds, not of an early rejection.

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
| Machine | Ryzen AI 9 HX 370, 24 threads, 15 GB RAM, Arch Linux x86_64 under WSL2 |
| Acceleration | none (CPU only) |
| Peak prover memory | about 8.9 GB (`r0vm`) |
| Guest cycles | 9,437,184 |
| Total wall clock, prove + compress | 1,302 s and 1,506 s over two runs |
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

## GPU

Same model, same machine, with `--features cuda` on an RTX 4070 Laptop (8 GB,
compute capability 8.9):

| Step | Result |
| ---- | ------ |
| Building the CUDA kernels | 8 min 27 s, targeting `sm_89` |
| zkVM proving on the GPU | ran to completion, 98 to 100% utilisation, 7.9 GB VRAM |
| Native Groth16 wrap | **failed**: illegal memory access inside `sppark` |

There is no end-to-end GPU number to report, because the run never produced a
bundle. The witness computation before the crash took 18 seconds, against a
1,302 second total on the CPU, which hints at what the path could be worth if
the memory problem is solved on a larger card. That hint is not a measurement.

Note also that `-arch` matters enormously. With nothing set, nvcc targets
`sm_52` and one rv32im kernel did not finish compiling in over an hour; with
`-arch=sm_89` the whole build takes eight minutes.

## Not yet measured

- Proving time per model family (tree, MLP) under real mode
- Remote proving latency, once a Boundless client exists
- End-to-end latency on testnet, which needs on-chain verification (issue #84)
