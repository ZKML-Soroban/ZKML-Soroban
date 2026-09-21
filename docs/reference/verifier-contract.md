---
title: "Verifier contract"
description: "zkml-verifier entrypoints, proof and public input encoding, storage, events, and TTL policy."
icon: "file-contract"
---

`zkml-verifier` is a Soroban contract (`soroban-sdk` 27) that verifies Groth16
proofs of ML inference with the BN254 host functions from CAP-0074.

| Property                  | Value                                   |
| ------------------------- | --------------------------------------- |
| Interface `VERSION`       | `6`                                     |
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
| `verify_receipt(seal: Bytes, journal: Bytes)` | none | `Result<(), VerificationError>` | Verifies a RISC Zero Groth16 receipt, enforces the nullifier, records the result, emits `verified`. |
| `set_risc0_config(config: Risc0Config)` | admin | `()` | Registers the guest image id, control root, BN254 control id and seal selector. |
| `get_risc0_config()` | none | `Risc0Config` | The registered configuration. Panics if never set. |
| `set_risc0_vk(vk: VerificationKey)` | admin | `()` | Registers RISC Zero's universal verifying key. Rejects any key without six `ic` points. |
| `claim_digest(image_id: Bytes, journal: Bytes)` | none | `Bytes` | Recomputes the digest a receipt commits to, so off-chain tools can check agreement. |

### Verifying a RISC Zero receipt

`verify_receipt` takes the two fields of a
[v2 bundle](/reference/bundle-format) unchanged. Set it up with the values
`zkml-prover export-vk` prints:

```bash
cargo run -p zkml-prover --features zkvm -- export-vk --format soroban-args
```

Those values are pinned to a RISC Zero version **and** to a guest build. Change
either and they must be re-exported, or every proof fails.

Two keys are registered, not one. `initialize` takes the key for the
native-circuit path, which has five `ic` points for four public inputs;
`set_risc0_vk` takes RISC Zero's universal key, which has six for five. They are
not interchangeable, and `set_risc0_vk` refuses the wrong length rather than
letting it fail later as a pairing error.

Checks run cheapest first: the seal's length and selector, then the journal's
layout, then the registered model hash, and only then the pairing.

The journal is not a public input of the proof. It is bound through the claim
digest, which the contract recomputes with `env.crypto().sha256()` using the
same arithmetic the prover runs. That is why a receipt attributed to a different
guest, or checked against a different control root, fails the pairing rather
than any cheaper check.

The three proof points are validated before the host sees them, because the
BN254 host functions trap on a point they cannot parse, which aborts the
transaction without saying why. A corrupted A, B or C returns
`MalformedProofA`, `MalformedProofB` or `MalformedProofC` instead. Every
single-byte change to the seal or the journal returns a typed error; a test
tries all 356 of them. This costs 2.4% of a verification.

G1 uses the host's `g1_is_on_curve` after a range check, since that function
traps on an unreduced coordinate. G2 has no host function, so its curve
equation is checked in plain Rust by `zkml_common::bn254`, tested against
`ark-bn254` on thousands of points.

<Note>
The G2 check does not test subgroup membership. A point on the curve but
outside the subgroup still traps in the pairing, so it is still rejected, only
without a typed error. Corruption does not produce such a point: a changed byte
lands back on the curve with probability about `1/p`. It takes a crafted one.
</Note>

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

Measured on the compiled WASM, `verify_inference` costs 30.2M CPU instructions
and `verify_receipt` 33.3M, against a 100M limit per transaction. See
[Benchmarks](/reference/benchmarks).

## Known limitations

- `verify_inference` does not validate its proof points, so a corrupted one
  traps in the host there. `verify_receipt` does.
- Neither entry point has been exercised by a deployed contract on a live
  network yet.
- A `(model, input, output, class_label)` tuple can be recorded only once.
- `get_result` is a single global slot overwritten by the next verification.
- Pausing returns the generic `VerificationFailed`.

See [Known limitations](/security/known-limitations).
