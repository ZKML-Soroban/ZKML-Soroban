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

The commitment is a constructor argument, so compute it before deploying. It
is hiding: it is computed under the model's opening, 32 secret random bytes
created once for this registration. Keep the opening file private and keep it,
since every proof for this contract needs it
([Commitments](/concepts/commitments#openings)).

```bash
openssl rand -hex 32 > model.opening
MODEL_HASH=$(cargo run -q -p zkml-prover -- commit examples/models/kyc_tree.json --opening model.opening)
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
  prove examples/models/kyc_tree.json -i "0.6,0.4,0.0" --opening model.opening \
  --groth16 -o bundle.json

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
creates and funds the identity if needed, generates the model's opening in
`target/kyc-demo/model.opening` unless `--opening` names one, deploys with the
constructor and no native key, registers the guest, proves the KYC model and
submits the receipt.

```bash
examples/kyc-demo/deploy.sh
```

Proving and exporting need x86_64 Linux with Docker. On another machine, make
the bundle and the `export-vk --format json` file on such a host and pass them
with `--bundle` and `--risc0-params`; the rest needs only cargo, Python and the
stellar CLI. `--help` lists the options and environment variables. See
[KYC demo](/guides/kyc-demo).

## Live on testnet

### Demo runner, 2026-11-24

`zkml-demo` deployed a contract `VERSION` 11 verifier, registered the RISC
Zero 3.1.0 parameters and the bundle's guest, submitted the golden receipt
(`credit_lr`, input `0.5,0.2,0.9,0.1`, hiding commitments) and read the record
back, equal to the native prediction:

| Item | Value |
| ---- | ----- |
| Contract | [`CA4MMQA6B72YP2Z3Y7IPF5GP6NFXYNPS6HC7HA74YN3UEVDJZIWXZWHB`](https://stellar.expert/explorer/testnet/contract/CA4MMQA6B72YP2Z3Y7IPF5GP6NFXYNPS6HC7HA74YN3UEVDJZIWXZWHB) |
| Guest image id | `fcf4b6b101f93c6bb7b6b89e778c494d113e55c0e6a34129de6b30257617f557` |
| `verify_receipt` | [`ba878e8d...`](https://stellar.expert/explorer/testnet/tx/ba878e8d651dc953079891a7fe3d349f2c2646d085d249f4b12f091838c50713), fee 784,460 stroops (0.078 XLM) |
| Recorded | `class_label` 1, `output` 34865 (0.532) |
### KYC tree with real proving, 2026-11-24

`zkml-demo` with the `groth16` feature proved the KYC tree
(`examples/models/kyc_tree.json`, input `0.6,0.4,0.0`, a fresh opening) on the
machine in [Benchmarks](/reference/benchmarks#proving), deployed a new verifier
and verified it. The recorded `class_label` is the risk tier, equal to native
inference.

| Item | Value |
| ---- | ----- |
| Contract | [`CDGMEKFBWWEYYCX6SPSZFVGE6PDVRKYCFLNQAJVIQUEWHMPURQ3PB4NC`](https://stellar.expert/explorer/testnet/contract/CDGMEKFBWWEYYCX6SPSZFVGE6PDVRKYCFLNQAJVIQUEWHMPURQ3PB4NC) |
| `verify_receipt` | [`6fec43d7...`](https://stellar.expert/explorer/testnet/tx/6fec43d7d53b1dd5075009333e51a7fd5fb9b87c7b1351ac2ef81ceb9310f766), fee 782,540 stroops (0.078 XLM) |
| Recorded | `class_label` 1, `output` 65536 (tier 1) |
| Stages | prove 3,354.8 s, deploy 16.6 s, submit 4.4 s |
| Criteria | proof 356 bytes: pass; on-chain result equals native: pass; under 60 s end to end: fail |

### Earlier runs, paused

<Warning>
The two contracts below are paused since 2026-11-24. They were registered with
RISC Zero 3.0.x parameters, which accept forged receipts (RISC Zero
GHSA-g6qg-9j8p-7j87; ours, GHSA-hmx6-94gg-9r4q). A verifier deployed today
uses the 3.1.0 parameters `export-vk` prints from `0.0.5` on.
</Warning>

Both runs below predate `0.0.4`, so their model commitments are the earlier,
deterministic ones; see [Commitments](/concepts/commitments#openings).

#### KYC tree, 2026-11-20

The script ran end to end with its defaults: the KYC tree
(`examples/models/kyc_tree.json`) on input `0.6,0.4,0.0`, proved with the
reproducible guest of release `0.0.3` and verified by a new `VERSION` 10
contract.

| Item | Value |
| ---- | ----- |
| Contract | [`CB5QQOVVNKHLJIVBY7YEVGNONFRETNPXHJLKBKOSX62WUBKX2RMYL23Y`](https://stellar.expert/explorer/testnet/contract/CB5QQOVVNKHLJIVBY7YEVGNONFRETNPXHJLKBKOSX62WUBKX2RMYL23Y) |
| WASM hash | `78553d40232a758e3989b20a944331f90aac97b5f94ca84581aa128836c061db`, already on the network from the first run |
| Guest image id | `618662ee2f9615d58b6ac5bacc874da9a1c3f7aec93109a30a820d892744e085` |
| Model hash | `95fb9a46c01fcf360cdaf66fcb1611aaf3111c1dd12a3b5db421b1f8882f4107` |

| Step | Transaction |
| ---- | ----------- |
| Deploy and construct | [`b8b6a48d...`](https://stellar.expert/explorer/testnet/tx/b8b6a48d98ed51e7231dcef6dc5642a90af03c7981b3aa52fcc8861fac2c00b7) |
| `set_risc0_config` | [`4dc912c6...`](https://stellar.expert/explorer/testnet/tx/4dc912c6854da31673f1ca83cf1c1edaca24e66bd8dac2d3bcc4c56a51d8748a) |
| `set_risc0_vk` | [`5a69226b...`](https://stellar.expert/explorer/testnet/tx/5a69226b14fbe34febf43a57c715a27120f3a108a32157b01497d34e7eaad7bd) |
| `verify_receipt` | [`f96d7203...`](https://stellar.expert/explorer/testnet/tx/f96d720353778635aaa552f096b7513a30761a053802217a2b5c9f673bd93bbd) |

The verification landed in ledger 5,833,131 and recorded output `65536`, risk
`1.0` in Q16.16, the high-risk branch, with `class_label` `0` as for every
tree. It declared 30,940,787 CPU instructions and was charged 778,696 stroops
(about 0.078 XLM). Proving took 2,727 seconds on a laptop CPU.

#### First receipt, 2026-11-17

The script deployed the `VERSION` 10 contract and verified the
golden receipt of the time: a real Groth16 proof of
`examples/models/credit_lr.json` on input `0.5,0.2,0.9,0.1`, from the
reproducible guest just before the `0.0.3` version bump. The bump changed the
image id to `618662ee...`, so this contract accepts new receipts only after
`set_risc0_config` registers it.

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
