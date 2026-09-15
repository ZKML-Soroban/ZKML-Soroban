---
title: "Commitments"
description: "Poseidon commitments that bind a proof to a model and an input, plus the nullifier scheme."
icon: "fingerprint"
---

Every proof is bound to a specific model and a specific input through 32-byte
Poseidon commitments over the BN254 scalar field.

- **Model commitment:** Poseidon over the quantized model parameters. Computed
  once (`zkml-prover commit`) and stored on-chain at `initialize`.
- **Input commitment:** Poseidon over the quantized input features. Carried in
  the public inputs of every proof.

The contract does not recompute Poseidon on-chain. It checks that the
`model_hash` public input equals the stored commitment, and the Groth16 proof
binds all public inputs, including `input_hash`, to the proven computation.

## Parameters

| Parameter      | Value                                                       |
| -------------- | ----------------------------------------------------------- |
| Field          | BN254 Fr, `r = 0x30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001` |
| Permutation    | circomlib-compatible Poseidon, width `t = 3` (`light-poseidon`, `new_circom(2)`) |
| S-box          | `x^5`                                                       |
| Rounds         | 8 full, 57 partial                                          |
| Output encoding| 32 bytes, little-endian (`to_bytes_le`)                     |

These are the circomlib BN254 parameters, which are also the defaults of
`rs-soroban-poseidon` (CAP-0075).

## Element encoding

Each `i64` element maps injectively into Fr:

- `v >= 0` maps to `Fr::from(v as u64)`
- `v < 0` maps to `r - |v|`

So `+w` and `-w` always produce different field elements.

## Construction

<Frame>
  <img src="/diagrams/05-commitments.svg" alt="Poseidon commitment construction" />
</Frame>

`poseidon_commit(elements, domain)` chains fixed-arity Poseidon calls over chunks
of 2 elements:

```text
state = Fr(domain)
for (i, chunk) in elements.chunks(2):
    x = pad_with_zeros(chunk, 2)
    x[0] = x[0] + state + Fr(i)
    state = Poseidon_2(x[0], x[1])
commitment = le_bytes(state)
```

The chunk index and the previous state are mixed into the first element of each
chunk, which makes the result order-sensitive.

### Public functions

| Function          | Domain | Used by                                            |
| ----------------- | ------ | -------------------------------------------------- |
| `commitment_hash` | `0`    | `model_commitment`, `input_commitment`, the zkVM guest, the CLI |
| `commit_i64`      | `0`    | `bundle_id`                                        |
| `commit_model`    | `1`    | Available, not used by the proving path            |
| `commit_inputs`   | `2`    | Available, not used by the proving path            |

> **Known limitation.** The proving path (host and guest) uses
> `commitment_hash` with domain `0` for both models and inputs, so model and
> input commitments currently share one domain. `commit_model` and
> `commit_inputs` implement the intended separation but are not wired in, and
> an empty element list returns the domain value without hashing. Unifying on
> the domain-separated functions is tracked as follow-up work. See
> [Known limitations](/security/known-limitations).

## Model serialization

`model_elements(model)` flattens parameters in this order (all fixed-point
values as their raw `i64`):

**Logistic regression**

1. `weights[0..n]`
2. `bias`
3. `decision_threshold`

**Decision tree**

1. `num_features`
2. For each node in index order:
   - split: `feature_index`, `threshold`, `left`, `right`
   - leaf: `value`

**Tiny MLP**, for each layer in order:

1. `weights` (row-major)
2. `biases`
3. `input_size`
4. `output_size`

**Inputs:** `features[0..n]` in model feature order.

## Public input byte order

All public inputs are little-endian: commitments come from `to_bytes_le`,
`output` and `class_label` from `i64::to_le_bytes`. Soroban's `U256` parses
big-endian only, so the verifier zero-extends each field to 32 bytes and
reverses it before converting to Fr. Any prover integration must keep emitting
little-endian public inputs.

See the full [public input layout](/reference/verifier-contract#public-inputs).

## Nullifier scheme

To block replays, the verifier derives:

```text
nullifier = SHA-256(public_inputs)    // all 80 bytes
```

and stores it in persistent storage under the key `("nullifier", nullifier)`
with TTL extended to `env.storage().max_ttl()`. A second submission with the
same `(model_hash, input_hash, output, class_label)` returns
`ProofAlreadyUsed`. The nullifier is written only after the pairing check
passes.

Off-chain services can predict the nullifier:

```rust
use sha2::{Digest, Sha256};

let mut pi = Vec::with_capacity(80);
pi.extend_from_slice(&model_hash);          // 32 bytes
pi.extend_from_slice(&input_hash);          // 32 bytes
pi.extend_from_slice(&output.to_le_bytes());      // 8 bytes
pi.extend_from_slice(&class_label.to_le_bytes()); // 8 bytes
let nullifier = Sha256::digest(&pi);
```

Because the nullifier depends only on public inputs, the same inference cannot
be recorded twice, even by a different submitter.

## Merkle tree

`zkml_common::merkle` provides a binary Merkle tree over commitments with
domain-separated leaf (`10`) and internal (`11`) hashing, `merkle_root`,
`generate_proof`, and `verify_proof`. It is intended for committing to chunked
model parameters so a single chunk can be opened later.

## Tests

Snapshot tests (`insta`) in `crates/zkml-common/src/commitment.rs` and
`merkle.rs` pin the digests for representative models, inputs, and trees.
Any change to the scheme changes the snapshots and must be reviewed as a
consensus-critical change.

## References

- [CAP-0075: Poseidon hash functions](https://stellar.org/protocol/cap-0075)
- [rs-soroban-poseidon](https://github.com/stellar/rs-soroban-poseidon)
- [circomlib Poseidon](https://github.com/iden3/circomlib/blob/master/circuits/poseidon.circom)
- Grassi et al., "Poseidon: A New Hash Function for Zero-Knowledge Proof Systems", 2021
