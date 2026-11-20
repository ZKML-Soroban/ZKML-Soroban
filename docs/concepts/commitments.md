---
title: "Commitments"
description: "Hiding Poseidon commitments that bind a proof to a model and an input, the opening that keeps them hiding, and the nullifier scheme."
icon: "fingerprint"
---

Every proof is bound to a specific model and a specific input through 32-byte
Poseidon commitments over the BN254 scalar field. Both are hiding: they are
computed under a secret **opening**, so they reveal nothing about the model or
the input to anyone who does not hold it.

- **Model commitment:** Poseidon over the quantized model parameters, under the
  model's opening. Computed once (`zkml-prover commit --opening FILE`) and
  stored on-chain by the contract constructor at deploy, or later by
  `set_model_hash`.
- **Input commitment:** Poseidon over the quantized input features, under the
  same opening and a different domain. Carried in the public inputs of every
  proof.

The contract does not recompute Poseidon on-chain. It checks that the
`model_hash` public input equals the stored commitment, and the Groth16 proof
binds all public inputs, including `input_hash`, to the proven computation.

## Openings

A commitment that is only a hash of public-format data is binding but not
hiding. The parameters of a small model, a tree with a few round thresholds or
a logistic regression with a handful of weights, can be recovered from its
public commitment by hashing candidate models until one matches, and so can
input features drawn from a small range, such as an age or a yes/no flag.
Commitments made before `0.0.4` work this way.

Since `0.0.4`, both commitments start from a 32-byte opening:

- **One per registration.** Generate it when the model is registered, from a
  cryptographically secure source, and keep it with the model:

  ```bash
  openssl rand -hex 32 > model.opening
  ```

  The CLI reads it as 64 hex characters and refuses an all-zero value.
  `*.opening` files are ignored by git in this repository.
- **Needed for every proof.** The zkVM guest takes it as a private input,
  recomputes both commitments with it, and never writes it to the journal. A
  proof made with another opening carries another `model_hash` and fails the
  contract's model check.
- **Kept private.** Whoever holds the opening can test guesses against the
  commitments again, so it belongs with the model, not with the proofs. Losing
  it means registering the model again under a new one.

The same input under the same opening always gives the same input commitment.
That is what lets the [nullifier](#nullifier-scheme) refuse a second recording
of one inference, and it also means an observer can tell that two proofs used
the same input, without learning the input.

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

A commitment chains fixed-arity Poseidon calls over chunks of 2 elements. The
hiding commitments start the chain from a hash of the domain and the opening,
read as a little-endian integer reduced modulo `r`:

```text
state = Poseidon_2(Fr(domain), Fr(opening))
for (i, chunk) in elements.chunks(2):
    x = pad_with_zeros(chunk, 2)
    x[0] = x[0] + state + Fr(i)
    state = Poseidon_2(x[0], x[1])
commitment = le_bytes(state)
```

Every later state depends on the first, so the commitment cannot be recomputed
for a guessed model without the opening. The chunk index and the previous state
are mixed into the first element of each chunk, which makes the result
order-sensitive. The deterministic functions run the same loop from
`state = Fr(domain)`.

### Public functions

| Function          | Domain | Hiding | Used by                                     |
| ----------------- | ------ | ------ | ------------------------------------------- |
| `commit_model`    | `3`    | Yes    | `model_commitment`, the zkVM guest, the CLI |
| `commit_inputs`   | `4`    | Yes    | `input_commitment`, the zkVM guest          |
| `commitment_hash` | `0`    | No     | Generic hashing                             |
| `commit_i64`      | `0`    | No     | `bundle_id`, the Merkle tree                |

Domains `1` and `2` belonged to the earlier, non-hiding `commit_model` and
`commit_inputs`; the hiding versions take new tags so the two schemes never
produce the same value. Up to `0.0.3` the proving path used `commitment_hash`
for both models and inputs.

## Model serialization

`model_elements(model)` flattens parameters in this order (all fixed-point
values as their raw `i64`). The stream is injective: it starts with the model
kind and length-prefixes every variable-length part, so two models of
different shape cannot flatten to the same sequence, and tree nodes carry a tag
so a leaf value cannot be read as a split's fields. This layout is new in
`0.0.2`; a model hash computed with `0.0.1` does not match.

Every model starts with its kind, the same number the journal carries:
`0` decision tree, `1` logistic regression, `2` tiny MLP.

**Logistic regression**

1. kind (`1`)
2. number of weights, then `weights[0..n]`
3. `bias`
4. `decision_threshold`

**Decision tree**

1. kind (`0`)
2. `num_features`
3. number of nodes
4. For each node in index order:
   - split: tag `0`, `feature_index`, `threshold`, `left`, `right`
   - leaf: tag `1`, `value`

**Tiny MLP**

1. kind (`2`)
2. number of layers
3. For each layer in order: number of weights, `weights` (row-major), number
   of biases, `biases`, `input_size`, `output_size`

**Inputs:** `features[0..n]` in model feature order, with no length prefix. The
hash pads the last chunk with zeros, so `[x]` and `[x, 0]` commit to the same
value; inference refuses an input whose length does not match the model, so
this cannot change a result. See [Known limitations](/security/known-limitations).

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
`merkle.rs` pin the digests for representative models, inputs, and trees, with
a fixed public opening. Any change to the scheme changes the snapshots and must
be reviewed as a consensus-critical change. A test also checks that no plain
hash of a model matches its hiding commitment, which is the property the
opening exists for.

## References

- [CAP-0075: Poseidon hash functions](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0075.md)
- [rs-soroban-poseidon](https://github.com/stellar/rs-soroban-poseidon)
- [circomlib Poseidon](https://github.com/iden3/circomlib/blob/master/circuits/poseidon.circom)
- Grassi et al., "Poseidon: A New Hash Function for Zero-Knowledge Proof Systems", 2021
