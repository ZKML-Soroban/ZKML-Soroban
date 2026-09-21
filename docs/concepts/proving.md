---
title: "Proving pipeline"
description: "How inference is proven with the RISC Zero zkVM and compressed to a Groth16 proof."
icon: "shield-check"
---

Inference is proven with the RISC Zero zkVM. The guest runs the model and
commits a journal; the host proves that execution, compresses the STARK receipt
to a Groth16 proof, and packages the result as a verification bundle.

## Status

| Step                                                   | Status                                  |
| ------------------------------------------------------ | --------------------------------------- |
| Commitments (`model_commitment`, `input_commitment`)    | Implemented                             |
| Guest execution and STARK receipt (`generate_receipt`)  | Implemented, CI in dev mode             |
| Journal cross-check against native inference            | Implemented                             |
| STARK to Groth16 (`prove_groth16`)                      | Implemented (see platform support)      |
| Verification key export (`export-vk`)                   | Implemented                             |
| `VerificationBundleV2` with real proof bytes            | Implemented                             |
| On-chain verification of those bundles                  | Pending (issue #84)                     |
| Remote proving (Boundless)                              | Not implemented (returns an error)      |

<Warning>
Without a GPU, Groth16 compression shells out to a Docker image that runs the
Circom witness generator, and that image is published for x86_64 only (risc0
issue #1749). On any other platform `prove_groth16` fails immediately with
`ProveError::UnsupportedPlatform` instead of failing deep inside the prover.
RISC Zero shut down the Bonsai proving service in December 2025, so there is no
hosted fallback; the remote path is reserved for Boundless.
</Warning>

## Platform support

Two things vary by platform: how fast the zkVM proves, and whether the Groth16
wrap can run locally at all.

| Platform | Inference and commitments | zkVM proving | Groth16 wrap |
| -------- | ------------------------- | ------------ | ------------ |
| Linux x86_64, no GPU | yes | CPU | Docker (`--features groth16`) |
| Linux x86_64 + NVIDIA | yes | GPU (`--features cuda`) | native, no Docker |
| Linux aarch64 + NVIDIA | yes | GPU (`--features cuda`) | native, no Docker |
| Linux aarch64, no GPU | yes | CPU | not locally: the Docker image is x86_64 only |
| macOS (Apple Silicon) | yes | partly GPU (`--features metal`) | not practically: needs x86 emulation |
| macOS (Intel) | yes | CPU | Docker (`--features groth16`) |
| Windows | yes | through WSL2 | through WSL2 |

Why the wrap is the awkward part: `risc0-groth16` has two prover paths and picks
between them at compile time.

```rust
// risc0-groth16 3.0.5, src/prove/mod.rs
if #[cfg(feature = "cuda")] { cuda::shrink_wrap(..) } else { docker::shrink_wrap(..) }
```

The CUDA path bundles a Rust witness calculator and a CUDA Groth16 prover, so it
needs neither Docker nor x86_64. The Docker path pulls a published image that
only exists for x86_64. There is no Metal path at all, which is why Apple
Silicon cannot do the wrap locally.

Metal coverage is partial in the rest of the pipeline too, so do not expect
CUDA-like speedups on a Mac. Counting the kernels shipped in risc0 3.0.6:

| Crate | Metal kernels | CUDA kernels |
| ----- | ------------- | ------------ |
| `risc0-sys` (NTT, Poseidon2) | 7 | 7 |
| `risc0-circuit-recursion-sys` | 3 | 9 |
| `risc0-circuit-rv32im-sys` | 0 | 7 |
| `risc0-circuit-keccak-sys` | 0 | 24 |
| `risc0-groth16-sys` | 0 | 1 |

`risc0-circuit-rv32im` is the circuit that proves guest execution, and it has no
Metal kernels and no `metal` feature, so the main proving step stays on the CPU
on macOS. Metal accelerates the recursion circuit and the shared primitives
only.

### Windows

RISC Zero's toolchain targets Linux and macOS, so proving on Windows goes
through WSL2. Everything that does not need the zkVM (import, quantization,
inference, commitments, `commit` / `infer` / `validate` / `inspect`) runs
natively on Windows, and `cargo test --workspace` passes there.

That needs one thing, which `.cargo/config.toml` sets for you: Windows gives
the main thread a 1 MB stack where Linux and macOS give 8 MB, and the Poseidon
commitment path overflows 1 MB in an unoptimized build. Without the larger
stack, `zkml-prover commit` exits with `0xC00000FD` (stack overflow) on Windows
debug builds while working everywhere else.

### Choosing a feature

```bash
cargo build -p zkml-prover --features groth16   # CPU, Docker, x86_64 Linux
cargo build -p zkml-prover --features cuda      # NVIDIA GPU, no Docker needed
cargo build -p zkml-prover --features metal     # Apple Silicon, proving only
```

`cuda` needs the CUDA toolkit (`nvcc`) at build time, plus one runtime
component:

```bash
rzup install risc0-groth16   # needs rzup >= 0.5.0
```

That component carries the Groth16 proving key. The Docker path ships it inside
the image, so this is only needed for the native CUDA path. Without it the zkVM
proof succeeds and then compression fails with
`Missing required risc0-groth16 rzup component`, minutes into the run.

### Building the CUDA path

Four things bite in practice, and none of them are obvious from the error
messages:

| Symptom | Cause | Fix |
| ------- | ----- | --- |
| `calling a __host__ function("__assert_fail") from a __global__ function` | `sppark` calls `assert()` in device code, which nvcc rejects unless `NDEBUG` is defined. `cc-rs` does not define it, in debug or release | `NVCC_PREPEND_FLAGS=-DNDEBUG` |
| `the global scope has no "CUdevice"` in `/usr/include/cccl/...` | `risc0-sys` 1.5 cannot compile the CCCL 13 headers | Use CUDA 12.x |
| `identifier "__builtin_operator_new" is undefined` in `/usr/include/c++/16/...` | `risc0-sys` hardcodes `-ccbin=c++`, and nvcc 12.x cannot parse libstdc++ 16 headers | Put a `c++` that points at a supported g++ (14) first on `PATH` |
| `ptxas` runs for over an hour on one kernel | nvcc defaults to `sm_52` when nothing sets an architecture, and the rv32im kernels are huge | `NVCC_PREPEND_FLAGS=-arch=sm_<your cc>` |

Putting it together, on a machine with an Ada GPU (compute capability 8.9):

```bash
export NVCC_PREPEND_FLAGS="-DNDEBUG -arch=sm_89"
export PATH="$HOME/.cuda-ccbin:/opt/cuda/bin:$PATH"   # c++ -> g++-14
cargo build -p zkml-prover --features cuda
```

That build takes about 8 minutes. With the wrong architecture it does not
finish at all.

Under WSL2 install only the toolkit: the GPU driver comes from Windows through
`/dev/dxg`, and installing a Linux NVIDIA driver package inside WSL overwrites
the `libcuda` that WSL provides and breaks CUDA.

<Note>
Compression is memory-hungry on the GPU too. On an 8 GB card it uses about
7.9 GB of VRAM, which leaves almost nothing for the desktop and makes the whole
machine sluggish while it runs. Budget for that, or prove on a headless box.
</Note>

## Pinned versions

| Component                           | Version  |
| ----------------------------------- | -------- |
| `risc0-zkvm`, `risc0-build`         | `=3.0.6` |
| `cargo-risczero`, `r0vm` toolchain  | `3.0.6`  |

Exact pins reduce the risk from RISC Zero API changes. Compression itself lives
in `risc0-zkvm` (`ProverOpts::groth16()`), so there is no separate pin for it.

## Feature flags (`zkml-prover`)

| Feature   | Enables                                                                |
| --------- | ---------------------------------------------------------------------- |
| `zkvm`    | `generate_receipt`, `decode_journal`, `verify_bundle`, `export-vk`      |
| `groth16` | `prove_groth16` and `prove --groth16` (implies `zkvm`)                  |
| `cuda`    | NVIDIA GPU proving and the native Groth16 wrap (implies `groth16`)      |
| `metal`   | Apple Silicon GPU for recursion and primitives only (implies `zkvm`)    |
| `timing`  | Timing output for inference and proving steps                          |

## Steps

1. Compute the model and input commitments.
2. Build an executor environment with the `Model` and the input vector.
3. Prove guest execution with `default_prover().prove_with_opts(.., ProverOpts::groth16())`.
4. Verify the receipt against `ZKML_GUEST_ID`.
5. Decode the journal and cross-check every field against native inference and
   the native commitments. A mismatch is `ProveError::JournalMismatch`.
6. Encode the seal: the 4-byte selector followed by the 256-byte proof.
7. Package everything into a [`VerificationBundleV2`](/reference/bundle-format).

```bash
# Real proof, written to disk.
cargo run -p zkml-prover --features groth16 -- \
  prove examples/models/credit_lr.json -i 0.5,0.2,0.9,0.1 --groth16 -o bundle.json

# Verify it locally, with RISC Zero's own verifier.
cargo run -p zkml-prover --features zkvm -- verify-bundle bundle.json
```

## Guest journal

Committed by `methods/guest/src/main.rs` as a fixed 96-byte record
(`JournalV1`), so the layout is stable across versions:

| Field         | Type       | Meaning                                  |
| ------------- | ---------- | ---------------------------------------- |
| `model_kind`  | `u8`       | Which model family ran                    |
| `model_hash`  | `[u8; 32]` | `commitment_hash(model_elements(model))` |
| `input_hash`  | `[u8; 32]` | `commitment_hash(input raw values)`      |
| `output`      | `i64`      | Raw `FixedPoint::value` of the score      |
| `class_label` | `i64`      | Decision (see [Models](/concepts/models)) |

## What a Groth16 receipt actually proves

A RISC Zero Groth16 proof is verified against RISC Zero's universal verifying
key. Its five public inputs are not the journal fields:

```text
[control_root_lo, control_root_hi, claim_digest_lo, claim_digest_hi, bn254_control_id]
```

`claim_digest` is the digest of `ReceiptClaim::ok(image_id, journal)`, which
binds *which program ran* to *what it output*. A verifier that holds the journal
therefore has to recompute that digest. `zkml_common::risc0` implements that
arithmetic without depending on `risc0-zkvm`, so the `no_std` contract can do it
too; `crates/zkml-prover/tests/risc0_digests.rs` cross-checks every value
against the real RISC Zero crates.

Two details are easy to get wrong and are covered by those tests: the digests
are split into 128-bit halves after being byte-reversed, and `bn254_control_id`
is byte-reversed before it is read as a field element.

## Dev mode and real proving

| Mode | Environment        | Use                                   |
| ---- | ------------------ | ------------------------------------- |
| Dev  | `RISC0_DEV_MODE=1` | CI and local tests (fake receipts)    |
| Real | unset or `0`       | Manual, `#[ignore]` tests, releases   |

```bash
RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm
```

Dev mode produces no seal at all, so `prove_groth16` refuses to run under it
(`ProveError::DevModeHasNoSeal`) rather than emitting a bundle that looks real.

## Code layout

```text
crates/zkml-common   models, FixedPoint, inference, commitments, journal, risc0, bundle
methods/guest        zkVM guest (depends only on zkml-common)
crates/zkml-prover   host: generate_receipt, prove_groth16, verify_bundle, bundles
```

## Known limitations

- The contract does not verify these bundles yet; that is issue #84.
- Remote proving (`--backend boundless`) returns `ProveError::RemoteBackend`.
- Compression needs roughly 10 GB of RAM and takes minutes on a laptop.
- Model and input commitments share domain `0` in the proving path.
- The legacy `prove` output (without `--groth16`) carries public inputs but no
  proof. It prints a warning and must not be treated as evidence.
