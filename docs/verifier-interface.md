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
