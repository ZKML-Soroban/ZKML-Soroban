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
| `verify_receipt` (point checks + claim digest + L assembly + pairing) | 29,578,818 | 1,817,254 | 30% |
| `verify_inference` (point checks + L assembly + pairing) | 28,956,777 | 1,795,390 | 29% |
| `set_risc0_vk` (point checks + G2 subgroup check) | 22,743,662 | | 23% |

Both entry points check the three proof points and assemble L with one
multi-scalar multiplication, `g1_msm`. Verifying a RISC Zero receipt costs a
little more than the native-circuit path: it also rebuilds the claim digest
(seven SHA-256 host calls) and has five public inputs to the other's four.
Up to contract `VERSION` 10 `verify_inference` made one host call per
multiplication and per addition instead; moving it to `g1_msm` in `VERSION` 11
saved 2,049,781 instructions.

Registering a verifying key is an admin call, run once per key. Since
`VERSION` 11 it pairs the key's three G2 points so the host checks they are in
the prime-order subgroup, which no host function does on its own; that pairing
is most of its cost.

The receipt figure uses the golden fixture in `crates/zkml-verifier/testdata/`.

Reproduce with:

```bash
cargo build -p zkml-verifier --target wasm32v1-none --profile contract
cargo test -p zkml-verifier wasm_budget -- --ignored --nocapture
```

CI runs the same two commands in the `Verifier WASM` job. Each verification
test fails above 30.5M instructions, about 3% over the larger figure here, and
the key registration above 23.5M, so a change that moves the cost noticeably
has to come with re-measured numbers.

<Note>
Measure on the WASM, not natively. The ordinary contract tests run it as native
Rust, and the native test environment meters only host function calls: code
running inside the contract is free there and not free on chain. The native
tests undercount a receipt by a few million instructions. For the G2 point
check, which is pure Rust, the native figure was off by a factor of more than 3,000.
</Note>

### Checking proof points

Both entry points validate A, B and C before the pairing, so a corrupted proof
returns a typed error instead of aborting the transaction in the host. Measured
on the WASM when the checks were added to `verify_receipt`, they cost 776,552
instructions. Adding them to `verify_inference` in contract `VERSION` 10 moved
it from 30,208,647 to 31,006,558, 797,911 more. Refusing points at infinity in
`VERSION` 11 added 14,653 to `verify_receipt`.

A first version checked G2 with double-and-add multiplication and cost
24,855,241 instructions, a 76% increase, which is why it uses Montgomery
multiplication now. The native tests reported that version as costing 7,355.

Breakdown:

- **`verify_inference`:** point validation, one 4-point `g1_msm` and one G1
  addition for `L = IC[0] + sum(x_i * IC[i + 1])`, then one 4-pair BN254
  pairing. Its fixture
  uses non-degenerate G2 points from the soroban-env-host test suite, the G1
  generator for every IC entry, and `C = -L` so the product of pairings is 1.
- **`verify_receipt`:** point validation, seven SHA-256 calls for the claim
  digest, one 5-point `g1_msm` for L, then the same 4-pair pairing. Its fixture
  is a real receipt.

The native tests also assert generous thresholds, as a fast guard against large
regressions:

```bash
cargo test -p zkml-verifier test_verifier_accept_path_and_resource_budget -- --nocapture
```

## zkVM cycles

Proving cost follows the guest's cycle count. `zkml-prover cycles` executes the
guest without proving and reports it in seconds; CI compares both example
models with `crates/zkml-prover/cycles-baseline.json` on the reproducible guest
and fails when one grows by more than 5%. RISC Zero 3.1.0, guest `51517769...` (`0.0.6`):

| Model | Input | User cycles | Padded (what proving pays for) | Segments |
| ----- | ----- | ----------- | ------------------------------ | -------- |
| `credit_lr.json` (logistic regression, 4 features) | `0.5,0.2,0.9,0.1` | 13,836,264 | 14,680,064 | 14 |
| `kyc_tree.json` (decision tree, 3 features) | `0.6,0.4,0.0` | 23,268,536 | 24,641,536 | 24 |

## Native

`cargo bench -p zkml-common --bench core` and `cargo bench -p zkml-prover
--bench onnx` (criterion), on the machine in the proving table below, under
WSL2. Medians:

| Operation | Time |
| --------- | ---- |
| `FixedPoint::checked_mul` / `checked_div` | 1.05 ns / 3.82 ns |
| `dot`, 16 / 128 / 1024 elements | 4.78 ns / 30.6 ns / 241 ns |
| Model commitment: logistic regression (4 weights) | 89.7 µs |
| Model commitment: tree, depth 5 / depth 12 | 1.95 ms / 251 ms |
| Model commitment: MLP 8-8-1 / 64-64-10 | 814 µs / 41.9 ms |
| Input commitment: 10 / 100 / 1000 features | 107 µs / 882 µs / 8.73 ms |
| Inference, one row: logistic regression / tree depth 5 / tree depth 12 / MLP 8-8-1 | 9.2 ns / 10.5 ns / 27.4 ns / 113 ns |
| Inference, 100 rows: logistic regression / tree depth 5 / tree depth 12 / MLP 8-8-1 | 781 ns / 1.03 µs / 2.86 µs / 11.2 µs |
| ONNX import: linear classifier / skl2onnx tree / MLP | 589 ns / 3.62 µs / 1.55 µs |

Poseidon dominates the host side: a commitment costs tens of microseconds per
pair of elements, inference a few nanoseconds per feature. Native numbers on
shared CI runners are noisy, so CI only checks that the benchmarks build
(`clippy --all-targets`); the cycle count is the gated figure.

## Proving

Measured with `crates/zkml-prover/tests/groth16_bundle.rs::real_groth16_bundle_verifies`
on `examples/models/credit_lr.json` (logistic regression, 4 features):

| Step | Value |
| ---- | ----- |
| Machine | Ryzen AI 9 HX 370, 24 threads, 15 GB RAM, Arch Linux x86_64 under WSL2 |
| Acceleration | none (CPU only) |
| Peak prover memory | about 8.9 GB (`r0vm`) |
| Guest cycles | 14,680,064 with RISC Zero 3.1.0 (`0.0.5`); 11,010,048 with 3.0.6, and 9,437,184 before the 0.0.2 guest changes |
| Total wall clock, prove + compress | 1,986 s with RISC Zero 3.1.0; 1,302 s, 1,506 s and 1,452 s over three runs with 3.0.6 |
| Seal | 260 bytes (4-byte selector + 256-byte proof) |
| Journal | 96 bytes |
| Total on-chain payload | 356 bytes |

The KYC tree (`kyc_tree.json`, 24,641,536 padded cycles) took 3,355 s to prove
on the same machine in the demo runner's testnet run, about in proportion to
its cycles. Local proving is far from the roadmap's 60-second end-to-end
target; remote proving is the way there.

Reproduce it with:

```bash
ZKML_GUEST_DOCKER=1 RISC0_DEV_MODE=0 cargo test -p zkml-prover --features groth16 \
  --test groth16_bundle real_groth16 -- --ignored --nocapture
```

`ZKML_GUEST_DOCKER=1` builds the reproducible guest, the one the golden receipt
in `crates/zkml-verifier/testdata/` comes from.

Two things this table is not: a floor and a ceiling. The model is tiny, so
almost all of that time is the fixed cost of the zkVM and the recursion, not the
inference; a decision tree or a small MLP lands in the same range. And a machine
with more memory and a GPU-enabled prover is much faster. Treat 25 minutes as
what a laptop does, not as what the design costs.

The KYC decision tree (`examples/models/kyc_tree.json`, three features) took
21,004,288 guest cycles and 2,727 seconds to prove and compress on the same
machine, for the receipt verified on testnet on 2026-11-20.

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

- Proving time for an MLP under real mode
- Remote proving latency, once a Boundless client exists
- End-to-end latency on testnet. The on-chain half is measured: a real receipt
  verified in a testnet transaction declaring 30,944,431 instructions, charged
  733,897 stroops (see [Live on testnet](/guides/deployment#live-on-testnet)).
  The proving half is the 25 minutes above, so the 60-second Phase 1 target
  needs a faster prover, not a cheaper contract.
