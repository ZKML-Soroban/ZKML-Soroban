# zkml-prover

Off-chain ML import, fixed-point inference, and proof generation via the RISC
Zero zkVM, including STARK to Groth16 compression.

## Features

| Feature | Default | Purpose |
| ------- | ------- | ------- |
| `zkvm` | no | Build/link the guest ELF; `generate_receipt`, `verify_bundle`, `export-vk` |
| `groth16` | no | `prove_groth16` and `prove --groth16` (implies `zkvm`) |
| `timing` | no | Print step timings |

Default workspace builds do **not** require the RISC Zero toolchain.

## RISC Zero guest proving

Pinned versions (exact):

- `risc0-zkvm = 3.0.6`
- `risc0-build = 3.0.6`

### Install toolchain

```bash
curl -L https://risczero.com/install | bash
rzup install
# pin when needed:
# rzup install cargo-risczero 3.0.6
# rzup install r0vm 3.0.6
```

### Dev-mode tests (CI default)

Fake receipts, fast iteration:

```bash
RISC0_DEV_MODE=1 cargo test -p zkml-prover --features zkvm
```

### Real proofs

```bash
RISC0_DEV_MODE=0 cargo test -p zkml-prover --test zkvm_receipt real_proof -- --ignored --nocapture
```

Real proving is **not** run on every push. The Groth16 job in CI is manual
(`workflow_dispatch`).

### Skip guest rebuild

When iterating on host-only code with the toolchain present:

```bash
RISC0_SKIP_BUILD=1 cargo test -p zkml-prover
```

## Groth16 compression

Compression runs a Circom witness generator inside Docker, and that image is
published for x86_64 only (risc0 issue #1749). The platform and Docker are
checked before anything expensive starts, so an unsupported host fails in
milliseconds with a clear error rather than minutes later.

```bash
# Real bundle (minutes, ~10 GB of RAM, x86_64 Linux with Docker)
cargo run -p zkml-prover --features groth16 -- \
  prove examples/models/credit_lr.json -i 0.5,0.2,0.9,0.1 --groth16 -o bundle.json

# Verify it with RISC Zero's own verifier
cargo run -p zkml-prover --features zkvm -- verify-bundle bundle.json

# Constants the contract needs at initialize
cargo run -p zkml-prover --features zkvm -- export-vk --format soroban-args

# The end-to-end test
RISC0_DEV_MODE=0 cargo test -p zkml-prover --features groth16 \
  --test groth16_bundle real_groth16 -- --ignored --nocapture
```

`RISC0_DEV_MODE=1` produces fake receipts with no seal, so `prove_groth16`
refuses to run under it instead of emitting a bundle that looks real.

Remote proving (`--backend boundless`) is not implemented. RISC Zero shut down
Bonsai in December 2025; the replacement is Boundless or a self-hosted Bento
prover.

## Public proving API

- `generate_receipt(model, inputs) -> (Receipt, JournalV1)`: prove guest
  execution, verify the receipt, cross-check the journal against native
  inference and the native commitments.
- `prove_groth16(model, inputs, backend) -> ProveOutput`: the same, compressed
  to Groth16, with the seal, the journal, timings and cycle count.
- `bundle_from_output(&ProveOutput) -> VerificationBundleV2`: package it.
- `verify_bundle(&VerificationBundleV2)`: rebuild the receipt from the bundle
  and verify it, the same check the contract performs.
- `image_id() -> [u8; 32]`: the guest image id, as registered on-chain.
- `generate_proof(model, inputs) -> VerificationBundle`: deprecated. Public
  inputs with no proof.

Errors are `ProveError`, one variant per recoverable cause; see
`docs/reference/errors.md`.

## Shared inference

Inference lives in `zkml-common::inference` and is re-exported as
`zkml_prover::inference`. The guest depends only on `zkml-common` so host and
guest execute the same logic.
