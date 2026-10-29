---
title: "Testnet deployment"
description: "Build the verifier contract and deploy it on Stellar testnet with the stellar CLI, passing its constructor arguments."
icon: "cloud-arrow-up"
---

This guide deploys `zkml-verifier` to Stellar testnet with the current
[`stellar` CLI](https://developers.stellar.org/docs/tools/cli). The legacy
`soroban` CLI is deprecated and should not be used.

> **Status.** Building, deploying, and initializing work today. A real
> verification key and real proofs are not available yet (Groth16 compression
> and VK export are pending), so `verify_inference` cannot succeed against a
> production proof. Steps marked **Pending** depend on that work.

## 1. Prerequisites

```bash
rustup target add wasm32v1-none
stellar --version
```

Create and fund a testnet identity:

```bash
stellar keys generate deployer --network testnet --fund
stellar keys address deployer
```

## 2. Build the contract

```bash
cargo build -p zkml-verifier --target wasm32v1-none --profile contract
ls -l target/wasm32v1-none/contract/zkml_verifier.wasm
```

## 3. Compute the model commitment

The commitment is a constructor argument, so compute it before deploying.

```bash
MODEL_HASH=$(cargo run -q -p zkml-prover -- commit examples/models/kyc_tree.json)
echo "$MODEL_HASH"
```

## 4. Deploy and initialize in one transaction

The contract has a constructor, so the admin, the model commitment and the
verification key are passed to `deploy` and set atomically as the contract is
created. There is deliberately no separate `initialize` call: a public one left
a window between deploy and init in which anyone could initialize the contract
first, seize the admin role and register a verification key of their own. The
constructor closes that window.

The admin must sign the deploy. `vk`, the key for the native-circuit path
(`verify_inference`), is optional. A verifier for RISC Zero receipts, which is
what the prover produces today, is deployed without it:

```bash
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/contract/zkml_verifier.wasm \
  --source-account deployer \
  --network testnet \
  -- \
  --admin "$(stellar keys address deployer)" \
  --model_hash "$MODEL_HASH")
echo "$CONTRACT_ID"
```

Without a native key, `verify_inference` returns `NativeKeyNotConfigured`, and
the contract verifies receipts once the admin registers the RISC Zero values
(`set_risc0_config` and `set_risc0_vk`, printed by
`zkml-prover export-vk --format soroban-args`).

<Warning>
Do not deploy with a test or placeholder native key. There is no native circuit
yet, so no native key is safe: a key whose trapdoor is known, like one built
from generator points, lets anyone prove any public input against it. The
contract refuses keys with points at infinity, which accepted an all-zero proof
for any input, but it cannot detect a known trapdoor.
</Warning>

When a native circuit exists, pass its key as `--vk "$(cat vk.json)"`, or
register it later with `set_verification_key`. `vk` is a `VerificationKey`
struct whose fields are hex-encoded bytes: `alpha` (G1, 64 bytes), `beta`,
`gamma`, `delta` (G2, 128 bytes each), and `ic` (exactly 5 G1 points, 64 bytes
each). Every point must be on its curve and none may be the point at infinity.

```json
{
  "alpha": "<64-byte hex>",
  "beta": "<128-byte hex>",
  "gamma": "<128-byte hex>",
  "delta": "<128-byte hex>",
  "ic": ["<64-byte hex>", "<64-byte hex>", "<64-byte hex>", "<64-byte hex>", "<64-byte hex>"]
}
```

## 5. Verify an inference

**Pending:** requires a native circuit, its key registered as above, and a
Groth16 proof from it. Receipts from the prover go through `verify_receipt`
instead; see [the verifier contract](/reference/verifier-contract#verifying-a-risc-zero-receipt).

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source-account deployer \
  --network testnet \
  -- verify_inference \
  --proof_a "<64-byte hex>" \
  --proof_b "<128-byte hex>" \
  --proof_c "<64-byte hex>" \
  --public_inputs "<80-byte hex>"
```

## 7. Read the result

```bash
stellar contract invoke --id "$CONTRACT_ID" --network testnet --source-account deployer -- get_result
stellar contract invoke --id "$CONTRACT_ID" --network testnet --source-account deployer -- get_verification_count
stellar contract invoke --id "$CONTRACT_ID" --network testnet --source-account deployer -- version
```

## Operations

| Task                   | Call                                   |
| ---------------------- | -------------------------------------- |
| Rotate the key         | `set_verification_key --vk ...`        |
| Register a new model   | `set_model_hash --model_hash ...`      |
| Transfer admin         | `propose_admin` then `accept_admin`    |
| Emergency stop         | `set_pause --paused true`              |

All four require the current admin signature (`--source-account` must be the
admin).

## The demo script

`examples/kyc-demo/deploy.sh` builds the contract with the command above but
still calls the deprecated `soroban` CLI, does not pass `--admin`, uses a
`--verification_key` argument (the parameter is `vk`), and falls back to
placeholder values. Prefer the commands on this page until the script is
updated. See [KYC demo](/guides/kyc-demo).
