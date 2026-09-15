---
title: "Verifier contract"
description: "zkml-verifier entrypoints, proof and public input encoding, storage, events, and TTL policy."
icon: "file-contract"
---

`zkml-verifier` is a Soroban contract (`soroban-sdk` 27) that verifies Groth16
proofs of ML inference with the BN254 host functions from CAP-0074.

| Property                  | Value                                   |
| ------------------------- | --------------------------------------- |
| Interface `VERSION`       | `5`                                     |
| `MIN_PROTOCOL_VERSION`    | `25` (X-Ray)                            |
| Build                     | `cargo build -p zkml-verifier --target wasm32v1-none --profile contract` |
| Artifact                  | `target/wasm32v1-none/contract/zkml_verifier.wasm` |

## Entrypoints

| Function | Auth | Returns | Description |
| -------- | ---- | ------- | ----------- |
| `initialize(admin: Address, model_hash: Bytes, vk: VerificationKey)` | `admin` | `()` | Stores admin, model commitment, and verification key; sets counter to 0 and pause to false; bumps instance TTL. Panics with `contract is already initialized` on a second call. |
| `verify_inference(proof_a: Bytes, proof_b: Bytes, proof_c: Bytes, public_inputs: Bytes)` | none | `Result<(), VerificationError>` | Verifies the proof, enforces the nullifier, records the result, emits `verified`. |
| `get_result()` | none | `InferenceRecord` | Last verified record. Panics if none has been recorded. |
| `get_model_hash()` | none | `Bytes` | Registered model commitment. Panics before initialization. |
| `get_verification_count()` | none | `u32` | Number of successful verifications. |
| `get_admin()` | none | `Address` | Current admin. Panics before initialization. |
| `is_paused()` | none | `bool` | Pause flag. |
| `version()` | none | `u32` | Interface version. |
| `set_verification_key(vk: VerificationKey)` | admin | `()` | Rotates the verification key. |
| `set_model_hash(model_hash: Bytes)` | admin | `()` | Replaces the model commitment. |
| `set_admin(new_admin: Address)` | admin | `()` | Transfers admin rights. |
| `set_pause(paused: bool)` | admin | `()` | Pauses or resumes verification. |

## Types

```rust
pub struct VerificationKey {
    pub alpha: Bytes,     // G1, 64 bytes
    pub beta: Bytes,      // G2, 128 bytes
    pub gamma: Bytes,     // G2, 128 bytes
    pub delta: Bytes,     // G2, 128 bytes
    pub ic: Vec<Bytes>,   // G1 points, 64 bytes each; exactly 5 entries
}

pub struct InferenceRecord {
    pub model_hash: Bytes,   // 32 bytes
    pub output: Bytes,       // 8 bytes, i64 little-endian
    pub class_label: i64,
    pub verified_at: u32,    // ledger sequence
}
```

## Proof encoding

| Argument  | Size      | Encoding                                             |
| --------- | --------- | ---------------------------------------------------- |
| `proof_a` | 64 bytes  | G1 `x \|\| y`, each 32 bytes big-endian                |
| `proof_b` | 128 bytes | G2 `x.c1 \|\| x.c0 \|\| y.c1 \|\| y.c0`, big-endian (EIP-197 layout) |
| `proof_c` | 64 bytes  | G1 `x \|\| y`, big-endian                              |

## Public inputs

<Frame>
  <img src="/diagrams/04-public-inputs.svg" alt="80-byte public input layout and BN254 scalars" />
</Frame>

`public_inputs` must be exactly 80 bytes:

| Offset | Size | Field         | Encoding                                     |
| ------ | ---- | ------------- | -------------------------------------------- |
| 0      | 32   | `model_hash`  | Poseidon commitment, little-endian Fr        |
| 32     | 32   | `input_hash`  | Poseidon commitment, little-endian Fr        |
| 64     | 8    | `output`      | `i64` little-endian (raw Q16.16)             |
| 72     | 8    | `class_label` | `i64` little-endian                          |

Each field becomes one BN254 scalar: bytes are read little-endian,
zero-extended to 32 bytes, and reduced modulo `r`. The verification key must
therefore contain exactly `4 + 1 = 5` IC points.

Length errors:

- fewer bytes than the next field needs: `PublicInputsTooShort`
- trailing bytes after `class_label`: `InvalidPublicInputLength`

## Verification steps

<Frame>
  <img src="/diagrams/03-verify-inference.svg" alt="verify_inference checks in execution order" />
</Frame>

1. Fail with `ContractNotInitialized` if not initialized.
2. Fail with `VerificationFailed` if paused.
3. Deserialize `proof_a`, `proof_b`, `proof_c` (length-checked).
4. Parse the 80-byte public inputs.
5. Fail with `VerificationFailed` if `model_hash` differs from the stored value.
6. Deserialize the verification key.
7. Compute `L = IC[0] + sum(x_i * IC[i + 1])`; fail with
   `VerificationKeyLengthMismatch` if `ic.len() != 5`.
8. Run `pairing_check([-A, alpha, L, C], [B, beta, gamma, delta])`; fail with
   `VerificationFailed` if it does not hold.
9. Derive `nullifier = sha256(public_inputs)`; fail with `ProofAlreadyUsed` if
   it exists.
10. Store the nullifier, the `InferenceRecord`, and the incremented counter;
    bump instance TTL; emit `verified`.

Failed verifications do not modify state.

## Errors

See [Errors](/reference/errors) for the full table of `VerificationError` codes.

## Events

On success the contract publishes:

| Part   | Value                                          |
| ------ | ---------------------------------------------- |
| Topics | `("verified", model_hash: Bytes)`              |
| Data   | `(verified_at: u32, output: Bytes)`            |

Indexers can filter by `model_hash` topic. Admin setters emit logs only, not
events.

## Storage

**Instance storage**

| Key        | Value                      |
| ---------- | -------------------------- |
| `init`     | Initialization flag        |
| `admin`    | Admin `Address`            |
| `mdl_hash` | Model commitment           |
| `vk`       | `VerificationKey`          |
| `lst_res`  | Last `InferenceRecord`     |
| `vrf_cnt`  | Verification counter       |
| `paused`   | Pause flag                 |

**Persistent storage**

| Key                         | Value  |
| --------------------------- | ------ |
| `("nullifier", sha256(pi))` | `true` |

## TTL policy

Instance storage shares the contract instance lifetime. The contract calls
`extend_ttl(threshold, extend_to)` at the end of `initialize` and after every
successful `verify_inference`:

| Constant                 | Value                               |
| ------------------------ | ----------------------------------- |
| `DAY_IN_LEDGERS`         | `17_280`                            |
| `INSTANCE_TTL_THRESHOLD` | 30 days (`30 * 17_280` ledgers)     |
| `INSTANCE_TTL_EXTEND_TO` | 120 days (`120 * 17_280` ledgers)   |

`extend_ttl` is a no-op unless the remaining TTL is below the threshold. Failed
verifications do not bump TTL. Nullifier entries are extended to
`env.storage().max_ttl()` when written and are not renewed afterwards.

## Cost

A full `verify_inference` measures about 29.3M CPU instructions and 278 KB of
memory in the Soroban test budget. See [Benchmarks](/reference/benchmarks).

## Known limitations

- A RISC Zero Groth16 proof exposes RISC Zero's own public inputs (control root,
  claim digest), not this layout; Route A needs an adapted verifier.
- A `(model, input, output, class_label)` tuple can be recorded only once.
- `get_result` is a single global slot overwritten by the next verification.
- Pausing returns the generic `VerificationFailed`.

See [Known limitations](/security/known-limitations).
