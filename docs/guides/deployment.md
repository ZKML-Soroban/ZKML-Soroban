---
title: "Testnet deployment"
description: "Build the verifier contract and deploy it on Stellar testnet with the stellar CLI, passing its constructor arguments."
icon: "cloud-arrow-up"
---

This guide deploys `zkml-verifier` to Stellar testnet with the current
[`stellar` CLI](https://developers.stellar.org/docs/tools/cli). The legacy
`soroban` CLI is deprecated and should not be used.

> **Status.** The whole receipt path runs on testnet: a contract deployed with
> these commands verified a real Groth16 receipt on 2026-11-17. See
> [Live on testnet](#live-on-testnet) for the transactions.
> `examples/kyc-demo/deploy.sh` runs every step below in one go; see
> [The demo script](#the-demo-script).

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
(step 5).

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

## 5. Register the guest

`export-vk --format soroban-args` prints the two admin calls that register the
guest image id with RISC Zero's parameters and its universal verifying key.
Build it with `ZKML_GUEST_DOCKER=1`, so the image id is the reproducible one,
and prove with a build made the same way:

```bash
ZKML_GUEST_DOCKER=1 cargo run -q -p zkml-prover --features zkvm -- \
  export-vk --format soroban-args
```

Replace `<CONTRACT>` and `<ADMIN>` in the two commands it prints with
`$CONTRACT_ID` and `deployer`, add `--network testnet`, and run them.

## 6. Prove and verify a receipt

Prove on x86_64 Linux with Docker (about 25 minutes of CPU for a small model),
then submit the bundle's seal and journal, both hex:

```bash
ZKML_GUEST_DOCKER=1 RISC0_DEV_MODE=0 cargo run -q -p zkml-prover --features groth16 -- \
  prove examples/models/kyc_tree.json -i "0.6,0.4,0.0" --groth16 -o bundle.json

stellar contract invoke --id "$CONTRACT_ID" --source-account deployer --network testnet \
  -- verify_receipt \
  --seal "$(python3 -c 'import json; print(json.load(open("bundle.json"))["seal"])')" \
  --journal "$(python3 -c 'import json; print(json.load(open("bundle.json"))["journal"])')"
```

It returns the recorded `InferenceRecord` and emits `verified`. The model hash
in the journal must be the one passed to the constructor, and a receipt can be
recorded only once.

`verify_inference`, the native-circuit path, needs a native circuit and its
key, which do not exist yet.

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

`examples/kyc-demo/deploy.sh` runs steps 1 to 7 from the repository root: it
creates and funds the identity if needed, deploys with the constructor and no
native key, registers the guest, proves the KYC model and submits the receipt.

```bash
examples/kyc-demo/deploy.sh
```

Proving and exporting need x86_64 Linux with Docker. On another machine, make
the bundle and the `export-vk --format json` file on such a host and pass them
with `--bundle` and `--risc0-params`; the rest needs only cargo, Python and the
stellar CLI. `--help` lists the options and environment variables. See
[KYC demo](/guides/kyc-demo).

## Live on testnet

On 2026-11-17 the script deployed the `VERSION` 10 contract and verified the
golden receipt in `crates/zkml-verifier/testdata/`: a real Groth16 proof of
`examples/models/credit_lr.json` on input `0.5,0.2,0.9,0.1`, from the
reproducible guest.

| Item | Value |
| ---- | ----- |
| Contract | [`CDRSZLNZCBLLNPDIWFIOCP4N3VB6UWTZTGH7ACSAFILQGYRG255FAPH7`](https://stellar.expert/explorer/testnet/contract/CDRSZLNZCBLLNPDIWFIOCP4N3VB6UWTZTGH7ACSAFILQGYRG255FAPH7) |
| WASM hash | `78553d40232a758e3989b20a944331f90aac97b5f94ca84581aa128836c061db` |
| Guest image id | `13cb39ca2b0559a9f437ba1a20ed0655c3f475d35b485ab763a56567200dd712` |
| Model hash | `c78a5f2e102455f2b84cc0903435f26c87fe7b198119ac84322874b12409b521` |

| Step | Transaction |
| ---- | ----------- |
| Upload WASM | [`3a2598f1...`](https://stellar.expert/explorer/testnet/tx/3a2598f1cd71a685fc324e1c3c754d677d9233c85901fee6d329fb1f67b54531) |
| Deploy and construct | [`a84bfd38...`](https://stellar.expert/explorer/testnet/tx/a84bfd38dac48cfc9e32c495bcc22a9a15c130440dd885a513931a7119080c66) |
| `set_risc0_config` | [`7472ec5b...`](https://stellar.expert/explorer/testnet/tx/7472ec5bbfdcbc025eb501d6c0ecb3d456946c855e65428893d3886d41c39d39) |
| `set_risc0_vk` | [`ed95ecde...`](https://stellar.expert/explorer/testnet/tx/ed95ecded190c09130fd3e58c7dd22fd9b7bd1367768cfe5031ad68b53144f15) |
| `verify_receipt` | [`06e534c2...`](https://stellar.expert/explorer/testnet/tx/06e534c2ea9e168d339632f7b6b2a62a6cb685a89f67ac6b47bed59b6a1c4c7c) |

The verification landed in ledger 5,785,346 and recorded output `34865` (raw
Q16.16, 0.532) with `class_label` `1`, the same values the native inference
gives. The transaction declared 30,944,431 CPU instructions, as simulated, and
was charged 733,897 stroops (about 0.073 XLM), 100 of them the inclusion fee.
