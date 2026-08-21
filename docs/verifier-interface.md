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

## Instance TTL policy

Instance storage shares one lifetime with the contract instance. If that TTL
is not renewed, a quiet verifier is archived and cannot be invoked until it
is restored. The contract therefore bumps instance TTL in two places:

- at the end of `initialize` (the contract is now live)
- after every **successful** `verify_inference` (active use, no external keeper)

Named constants live next to the storage keys in `crates/zkml-verifier/src/lib.rs`:

| Constant | Value | Rationale |
| -------- | ----- | --------- |
| `INSTANCE_TTL_THRESHOLD` | 30 days (`30 * 17_280` ledgers) | Renew when remaining lifetime falls below a month. |
| `INSTANCE_TTL_EXTEND_TO` | 120 days (`120 * 17_280` ledgers) | Typical persistent rent floor; under the ~180-day network max. |

`extend_ttl` is a no-op unless the current TTL is below the threshold, so a
recently initialized or recently verified contract does not pay extra rent.
Failed verifications do not bump TTL. Persistent entries such as nullifiers
need their own policy and are out of scope here.
