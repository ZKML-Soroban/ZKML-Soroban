---
title: "Testnet deployment"
description: "Build, deploy, and initialize the verifier contract on Stellar testnet with the stellar CLI."
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

`vk` is a `VerificationKey` struct whose fields are hex-encoded bytes: `alpha`
(G1, 64 bytes), `beta`, `gamma`, `delta` (G2, 128 bytes each), and `ic` (a list
of exactly 5 G1 points, 64 bytes each). The admin must sign the deploy.

```bash
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/contract/zkml_verifier.wasm \
  --source-account deployer \
  --network testnet \
  -- \
  --admin "$(stellar keys address deployer)" \
  --model_hash "$MODEL_HASH" \
  --vk "$(cat vk.json)")
echo "$CONTRACT_ID"
```

`vk.json` shape:

```json
{
  "alpha": "<64-byte hex>",
  "beta": "<128-byte hex>",
  "gamma": "<128-byte hex>",
  "delta": "<128-byte hex>",
  "ic": ["<64-byte hex>", "<64-byte hex>", "<64-byte hex>", "<64-byte hex>", "<64-byte hex>"]
}
```

**Pending:** there is no tool yet that exports `vk.json` from the prover. Until
then, deployment can only use test keys.

## 5. Verify an inference

**Pending:** requires a Groth16 proof from the prover.

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
