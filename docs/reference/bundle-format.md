---
title: "Bundle format"
description: "The v2 verification bundle, the guest journal, the seal layout, and the legacy v1 shape."
icon: "box"
---

A verification bundle is the single artifact handed from the prover to the
verifier. There are two shapes: **v2**, produced by `prove --groth16`, which
carries a real proof, and the **legacy v1** shape, which carries public inputs
and nothing else.

## v2 (`VerificationBundleV2`)

Lives in `zkml_common::bundle`. Bytes are hex strings, not integer arrays, so a
bundle is readable and roughly half the size of the v1 JSON.

```json
{
  "version": 2,
  "proof_system": "Risc0Groth16",
  "image_id": "04de8993fd7f341c62fcab73aa007eb3d09e4c6882fd31db6a21d56b8612c5ca",
  "seal": "73c457ba2e0f32f8faf15e48...",
  "journal": "5a4b4d4c0100010073e881eda8...",
  "meta": {
    "prover_version": "0.0.1",
    "created_at": 1790002334,
    "cycles": 9437184,
    "timings": { "prove_and_compress_ms": 1172526, "total_ms": 1172573 }
  }
}
```

That is a real bundle for `examples/models/credit_lr.json` with inputs
`0.5,0.2,0.9,0.1`, trimmed for width. Its journal decodes to the same
`model hash 73e881ed...` that `zkml-prover commit` prints for that model, and to
`output 34865`, which is what `infer` prints.

| Field          | Meaning                                                             |
| -------------- | ------------------------------------------------------------------- |
| `version`      | `2`                                                                 |
| `proof_system` | `Risc0Groth16` (id `1` in the binary encoding)                      |
| `image_id`     | 32 bytes: which guest program ran                                    |
| `seal`         | 260 bytes: 4-byte selector followed by the 256-byte Groth16 proof    |
| `journal`      | 96 bytes: what the guest committed (see below)                       |
| `meta`         | Provenance and timings. Informational; nothing verifies against it.  |

`VerificationBundleV2::new` rejects a seal that is not 260 bytes and a journal
that does not decode, so a malformed bundle cannot be constructed by accident.

`to_bytes` / `from_bytes` give a compact binary encoding (magic `ZKMLBNDL`) for
transports where JSON is wasteful. `AnyBundle` parses either version, so a tool
can accept both and call `is_legacy()` to tell them apart.

## Journal (`JournalV1`)

The guest commits a fixed 96-byte record. The layout is versioned so a contract
can reject anything it was not compiled for.

| Offset | Size | Field         | Notes                                    |
| ------ | ---- | ------------- | ---------------------------------------- |
| 0      | 4    | magic         | `ZKML`                                   |
| 4      | 2    | version       | `1`, little-endian                       |
| 6      | 1    | `model_kind`  | tree `0`, logistic regression `1`, MLP `2` |
| 7      | 1    | reserved      | zero                                     |
| 8      | 32   | `model_hash`  | `commitment_hash(model_elements(model))` |
| 40     | 32   | `input_hash`  | `commitment_hash(input raw values)`      |
| 72     | 8    | `output`      | raw Q16.16 value, `i64` little-endian     |
| 80     | 8    | `class_label` | `i64` little-endian                       |
| 88     | 8    | reserved      | zero                                     |

## Seal

| Range      | Content                                                      |
| ---------- | ------------------------------------------------------------ |
| `[0, 4)`   | Selector: first four bytes of the verifier parameters digest  |
| `[4, 260)` | The Groth16 proof                                             |

The selector says which verifying key the proof was made for, so a verifier can
reject a proof from a different RISC Zero version instead of failing a pairing
check for no stated reason. `export-vk` prints the selector this binary
produces. The proof bytes themselves are 256 bytes:

| Range        | Point   | Encoding                                          |
| ------------ | ------- | ------------------------------------------------- |
| `[0, 64)`    | A (G1)  | `x \|\| y`, 32 bytes each, big-endian               |
| `[64, 192)`  | B (G2)  | `x.c1 \|\| x.c0 \|\| y.c1 \|\| y.c0`, big-endian        |
| `[192, 256)` | C (G1)  | `x \|\| y`, big-endian                              |

This matches the Ethereum precompile layout (EIP-196 / EIP-197), which is also
what Soroban's BN254 host functions (CAP-0074) read.

## Public inputs

Two different things are called public inputs, and confusing them is the
quickest way to a failing verification.

**The contract's 80-byte layout**, derived from the journal by
`JournalV1::public_inputs_bytes()`:

```text
model_hash (32) || input_hash (32) || output (8, LE) || class_label (8, LE)   = 80 bytes
```

This is what `verify_inference` and `verify_receipt` hash into the nullifier,
and what the model hash is checked against. See the
[verifier contract reference](/reference/verifier-contract#public-inputs).

**The Groth16 proof's five field elements**, which are what the pairing check
actually consumes:

```text
[control_root_lo, control_root_hi, claim_digest_lo, claim_digest_hi, bn254_control_id]
```

`claim_digest` is the digest of `ReceiptClaim::ok(image_id, journal)`. A
verifier recomputes it from the journal it was given with
`zkml_common::risc0::receipt_claim_ok_digest`, then builds the five scalars with
`groth16_public_inputs`. The journal is therefore bound to the proof even though
it is not a public input itself. See
[the proving pipeline](/concepts/proving#what-a-groth16-receipt-actually-proves).

## Verifying on chain

The contract's `verify_receipt(seal, journal)` takes the `seal` and `journal`
fields of a v2 bundle unchanged. It rebuilds the claim digest from the journal
and the registered image id, so the `image_id` field of the bundle is not sent:
a bundle from another guest fails the pairing. See
[the verifier contract](/reference/verifier-contract#verifying-a-risc-zero-receipt).

## Legacy v1 (`VerificationBundle`)

`zkml_common::proof`, produced by `prove` without `--groth16`. Byte arrays are
JSON arrays of integers and `proof.data` is empty.

```json
{
  "proof": { "data": [] },
  "public_inputs": {
    "model_hash": [/* 32 bytes */],
    "input_hash": [/* 32 bytes */],
    "output": [/* 8 bytes, i64 little-endian */],
    "class_label": 1
  }
}
```

<Warning>
A v1 bundle carries no proof. It is a description of what would be proven, not
evidence that it was. Never accept one as verification.
</Warning>

## Bundle id

`bundle_id(bundle)` derives a deterministic 32-byte identifier from
`model_hash`, `input_hash`, and `output` (via `commit_i64`). Off-chain services
can use it to index or de-duplicate submissions. It does not include
`class_label`, and it is not the on-chain nullifier (which is
`sha256(public_inputs)`, see [Commitments](/concepts/commitments#nullifier-scheme)).
