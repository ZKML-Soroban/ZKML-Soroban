---
title: "Quickstart"
description: "Build the workspace, run fixed-point inference, and export a verification bundle."
icon: "rocket"
---

## Prerequisites

- [Rust](https://rustup.rs/) stable (pinned in `rust-toolchain.toml`)
- The `wasm32v1-none` target, to build the verifier contract:

```bash
rustup target add wasm32v1-none
```

- Optional: the [Stellar CLI](https://developers.stellar.org/docs/tools/cli) for deployment
- Optional: the [RISC Zero toolchain](https://dev.risczero.com/api/zkvm/install) (`rzup`, version `3.0.6`) to run the zkVM guest

## Build and test

```bash
git clone https://github.com/ZKML-Soroban/ZKML-Soroban.git
cd ZKML-Soroban

cargo build --workspace
cargo test --workspace
```

The default build does not need the RISC Zero toolchain. zkVM proving is behind
the `zkvm` feature of `zkml-prover`.

## On Windows

The commands on this page work as-is in PowerShell, with two differences:

- Setting an environment variable for a single command uses `$env:` instead of
  a leading `VAR=value`. For example, running the zkVM guest in dev mode
  (see [below](#run-the-zkvm-guest-optional)) is:

  ```powershell
  $env:RISC0_DEV_MODE = "1"
  cargo test -p zkml-prover --features zkvm
  ```

- The `wasm32v1-none` target (`rustup target add wasm32v1-none`) installs the
  same way as on Linux or macOS; no extra Windows-specific setup is needed for
  it.

## Use the crates

`zkml-common` and `zkml-verifier` are published to crates.io starting with the
first release, `0.0.1`. Until that release is out, depend on the Git repository
instead. `zkml-prover` is not published yet (`publish = false`).

```toml
[dependencies]
zkml-common = "0.0.1"
```

## Run inference

Example models live in `examples/models/` in the [JSON exchange format](/guides/model-format).

```bash
cargo run -p zkml-prover -- infer examples/models/credit_lr.json -i "0.5,0.2,0.9,0.1"
```

```text
model commitment: 73e881eda85b98eef6a08eec16e3210330c6bd53a182eab7944c87d9c4fce710
output: 0.5319976806640625
output (raw Q16.16): 34865
```

## Export a verification bundle

```bash
cargo run -p zkml-prover -- prove examples/models/credit_lr.json \
  -i "0.5,0.2,0.9,0.1" -o bundle.json
```

> **Warning.** The Groth16 proof bytes in the bundle are empty until
> STARK-to-Groth16 compression is implemented. The bundle exercises the
> interface but will not verify against a live contract.

## Run the zkVM guest (optional)

With the RISC Zero toolchain installed, run the guest in dev mode (fast, fake
receipts):

```bash
RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm
```

## Build the verifier contract

```bash
cargo build -p zkml-verifier --target wasm32v1-none --profile contract
# output: target/wasm32v1-none/contract/zkml_verifier.wasm
```

## Next steps

- [CLI reference](/guides/cli)
- [Architecture](/concepts/architecture)
- [Testnet deployment](/guides/deployment)
