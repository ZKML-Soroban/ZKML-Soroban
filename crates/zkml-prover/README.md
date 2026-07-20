# zkml-prover

Off-chain ML import, fixed-point inference, and Phase 1 proof generation via
the RISC Zero zkVM.

## Features

| Feature | Default | Purpose |
| ------- | ------- | ------- |
| `zkvm` | no | Build/link the guest ELF and expose `generate_receipt` |
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

Real proving is **not** run in CI. STARK→Groth16 compression for on-chain
submission is tracked in issue #11.

### Skip guest rebuild

When iterating on host-only code with the toolchain present:

```bash
RISC0_SKIP_BUILD=1 cargo test -p zkml-prover
```

## Public proving API

- `generate_receipt(model, inputs) -> (Receipt, InferenceJournal)` — prove
  guest execution, verify the receipt, cross-check the journal against native
  `run_inference` and `commitment_hash` (Poseidon replacement: issue #13).
- `generate_proof(model, inputs) -> VerificationBundle` — public inputs with a
  placeholder Groth16 proof until issue #11.

## Shared inference

Inference lives in `zkml-common::inference` and is re-exported as
`zkml_prover::inference`. The guest depends only on `zkml-common` so host and
guest execute the same logic.
