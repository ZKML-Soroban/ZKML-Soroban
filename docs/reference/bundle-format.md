---
title: "Bundle format"
description: "VerificationBundle JSON, Groth16 proof bytes, and public input serialization."
icon: "box"
---

A `VerificationBundle` (`zkml_common::proof`) is the single artifact handed from
the prover to the verifier. It serializes to JSON with `serde_json`.

```json
{
  "proof": { "data": [/* 256 bytes: A || B || C */] },
  "public_inputs": {
    "model_hash": [/* 32 bytes */],
    "input_hash": [/* 32 bytes */],
    "output": [/* 8 bytes, i64 little-endian */],
    "class_label": 1
  }
}
```

Byte arrays are JSON arrays of integers. Produce and parse bundles with
`zkml_prover::prover::bundle_to_json` and `bundle_from_json`, or with the CLI
`prove` subcommand.

> The current prover emits `proof.data` as an empty array until Groth16
> compression is implemented.

## Proof bytes

`Groth16Proof.data` must be exactly 256 bytes:

| Range        | Point   | Encoding                                          |
| ------------ | ------- | ------------------------------------------------- |
| `[0, 64)`    | A (G1)  | `x \|\| y`, 32 bytes each, big-endian               |
| `[64, 192)`  | B (G2)  | `x.c1 \|\| x.c0 \|\| y.c1 \|\| y.c0`, big-endian        |
| `[192, 256)` | C (G1)  | `x \|\| y`, big-endian                              |

This matches the Ethereum precompile layout (EIP-196 / EIP-197). Split it into
`proof_a`, `proof_b`, and `proof_c` when calling `verify_inference`.

## Public inputs

`PublicInputs::to_bytes()` produces the contract layout:

```text
model_hash (32) || input_hash (32) || output (8, LE) || class_label (8, LE)   = 80 bytes
```

Pass the result as `public_inputs` to `verify_inference`. See the
[verifier contract reference](/reference/verifier-contract#public-inputs).

## Bundle id

`bundle_id(bundle)` derives a deterministic 32-byte identifier from
`model_hash`, `input_hash`, and `output` (via `commit_i64`). Off-chain services
can use it to index or de-duplicate submissions. It does not include
`class_label`, and it is not the on-chain nullifier (which is
`sha256(public_inputs)`, see [Commitments](/concepts/commitments#nullifier-scheme)).
