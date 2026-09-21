---
title: "Benchmarks"
description: "Native inference baselines and the measured on-chain verification budget."
icon: "gauge"
---

## On-chain verification budget

Measured on the compiled contract (`wasm32v1-none`, `contract` profile) with
the Soroban cost estimator, each on a proof that genuinely verifies:

| Operation | CPU instructions | Memory (bytes) | Share of the 100M limit |
| --------- | ---------------- | -------------- | ----------------------- |
| `verify_inference` (L assembly + pairing) | 30,198,101 | 1,662,342 | 30% |
| `verify_receipt` (point checks + claim digest + L assembly + pairing) | 33,251,381 | 1,711,079 | 33% |

Verifying a RISC Zero receipt costs 10.1% more than the native-circuit path.
The extra is the claim digest (eight SHA-256 host calls), one more scalar
multiplication, and checking the three proof points before the host sees them.
The receipt figure uses the golden fixture in `crates/zkml-verifier/testdata/`.

Reproduce with:

```bash
cargo build -p zkml-verifier --target wasm32v1-none --profile contract
cargo test -p zkml-verifier wasm_budget -- --ignored --nocapture
```

<Note>
Measure on the WASM, not natively. The ordinary contract tests run it as native
Rust, and the native test environment meters only host function calls: code
running inside the contract is free there and not free on chain. The native
tests report 30.7M for a receipt, which undercounts by 2.6M. For the G2 point
check, which is pure Rust, the native figure was off by a factor of more than 3,000.
</Note>

### Checking proof points

`verify_receipt` validates A, B and C before the pairing, so a corrupted proof
returns a typed error instead of aborting the transaction in the host. On the
WASM this costs 776,552 instructions, 2.4% of a verification.

A first version checked G2 with double-and-add multiplication and cost
24,855,241 instructions, a 76% increase, which is why it uses Montgomery
multiplication now. The native tests reported that version as costing 7,355.

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
- End-to-end latency on testnet: proof, submission and verification. On-chain
  verification exists and is measured above, but nothing has been deployed
