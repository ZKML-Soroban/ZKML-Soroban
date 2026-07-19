# Proof Generation Pipeline

Phase 1 generates proofs with the RISC Zero zkVM. Guest execution produces a
STARK receipt and a public journal; STARK→Groth16 compression for Soroban is
tracked separately (issue #11).

## Pinned versions

| Component | Version |
| --------- | ------- |
| `risc0-zkvm` | `=3.0.6` |
| `risc0-build` | `=3.0.6` |
| `cargo-risczero` / `r0vm` (toolchain) | `3.0.6` |

Exact pins reduce risk from RISC Zero API changes (see roadmap risks).

## Steps

1. Commit to the model parameters (`model_commitment` / `commitment_hash`) and
   the input features (`input_commitment`).
2. Run inference inside the zkVM guest (`methods/guest`), producing a journal
   and a STARK receipt.
3. Host verifies the receipt against the guest image ID and cross-checks the
   journal against native `run_inference`.
4. *(Issue #11)* Convert the STARK receipt to a Groth16 SNARK (Bonsai or local).
5. Package the proof and public inputs into a `VerificationBundle`.

## Guest journal (public inputs)

Committed in order by the guest:

| Field | Type | Meaning |
| ----- | ---- | ------- |
| `model_hash` | `[u8; 32]` | `commitment_hash` over model parameters |
| `input_hash` | `[u8; 32]` | `commitment_hash` over input raw values |
| `output` | `i64` | Raw `FixedPoint::value` inference result |

## Dev mode vs real proving

| Mode | Env | Use |
| ---- | --- | --- |
| Dev | `RISC0_DEV_MODE=1` | CI and local tests (fast fake receipts) |
| Real | `RISC0_DEV_MODE=0` or unset | Manual `#[ignore]` test only |

## Shared code layout

```
zkml-common          models, FixedPoint, inference, commitment_hash
methods/guest        zkVM guest (depends only on zkml-common)
zkml-prover          host: generate_receipt, generate_proof
```

## Related issues

- #10 — guest execution + STARK receipt (this pipeline)
- #11 — STARK→Groth16 compression and bundle serialization
- #13 — Poseidon commitments (replace `commitment_hash` stand-in)
