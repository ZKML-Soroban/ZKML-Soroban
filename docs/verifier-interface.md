# Verifier Contract Interface

The `zkml-verifier` Soroban contract exposes the following entrypoints.

| Function                 | Description                                        |
| ------------------------ | -------------------------------------------------- |
| `initialize(model_hash)` | Register the model commitment. Call once.          |
| `verify_inference(...)`  | Verify a proof + public inputs, record the result. |
| `get_result()`           | Return the last `InferenceRecord`.                 |
| `get_model_hash()`       | Return the registered model commitment.            |
| `get_verification_count()` | Return the number of verified proofs.            |
| `version()`              | Return the contract interface version.             |

## Public inputs layout

`verify_inference` expects `public_inputs` to be the concatenation of:

1. `model_hash` (32 bytes)
2. `input_hash` (32 bytes)
3. `output` (remaining bytes)

## Storage layout

The contract uses instance storage with the following short-symbol keys:

| Key        | Value                                  |
| ---------- | -------------------------------------- |
| `init`     | Initialization flag (`bool`).          |
| `mdl_hash` | The registered model commitment.       |
| `lst_res`  | The last `InferenceRecord`.            |
| `vrf_cnt`  | Cumulative count of verified proofs.   |

Instance storage is used because every entry is small and is read on nearly
every call, so it benefits from being loaded together with the contract.

## Events

On a successful `verify_inference`, the contract emits a `verified` event so an
off-chain indexer can learn "which model produced which output" without polling
`get_result` (which only returns the most recent record and races against the
next verification).

| Field        | Position        | Type    | Description                                                        |
| ------------ | --------------- | ------- | ------------------------------------------------------------------ |
| `verified`   | topic[0]        | Symbol  | Constant event name.                                               |
| `model_hash` | topic[1]        | Bytes   | Poseidon commitment to the model (32 bytes). A topic so indexers can subscribe per model. |
| `verified_at`| data[0]         | u32     | Ledger sequence number at verification time.                       |
| `output`     | data[1]         | Bytes   | The inference output value. Kept in the data (not a topic) because it is variable-length. |

In `soroban-sdk` terms the publish call is:

```rust
env.events().publish(
    (symbol_short!("verified"), record.model_hash.clone()),
    (record.verified_at, record.output.clone()),
);
```

Consumers should filter on `topic[0] == "verified"` and may additionally filter
on `topic[1] == <model_hash>` to track a single model across deployments.
