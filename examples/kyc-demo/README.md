# KYC Demo: Provable ML Inference on Stellar Testnet

End-to-end provable KYC risk scoring with zkml-soroban: train a decision tree on
synthetic KYC data, deploy the verifier contract to testnet, prove a risk score,
and verify it on-chain.

Full walkthrough: [docs/guides/kyc-demo.md](../../docs/guides/kyc-demo.md).
Deployment with the `stellar` CLI: [docs/guides/deployment.md](../../docs/guides/deployment.md).

## Current status

| Step | Status |
| ---- | ------ |
| Synthetic dataset (`generate_dataset.py`) | Works |
| Training and ONNX export (`train_model.py`) | Works, but exports with `target_opset=12` |
| ONNX import into `zkml-prover` | Blocked until the export uses core opset 17 or later |
| Verifier contract with Groth16 (BN254) verification | Implemented |
| Poseidon model commitment | Implemented (`zkml-prover commit` for JSON models) |
| Contract build and deployment | Works with the `stellar` CLI (see the deployment guide) |
| STARK to Groth16 compression | Pending (returns "not yet implemented") |
| Verification key export | Pending |
| Demo runner (`zkml-demo`) | Skeleton only |

## Prerequisites

- Rust stable with the `wasm32v1-none` target (`rustup target add wasm32v1-none`)
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli) (the legacy `soroban` CLI is deprecated)
- Python 3 with the packages in `requirements.txt`

Create and fund a testnet identity:

```bash
stellar keys generate deployer --network testnet --fund
```

## 1. Generate the synthetic dataset

```bash
cd examples/kyc-demo
pip install -r requirements.txt
python generate_dataset.py
```

Creates `kyc_dataset.csv` with 1000 synthetic records.

## 2. Train and export the model

```bash
python train_model.py
```

Trains a decision tree (`max_depth=5`, risk tiers 0 low, 1 medium, 2 high) and
exports `kyc_decision_tree.onnx`.

**Known issue:** the script passes `target_opset=12`. The importer requires core
opset 17 or later, so update the export before importing the model.

## 3. Deploy the verifier

`deploy.sh` builds the contract with
`cargo build -p zkml-verifier --target wasm32v1-none --profile contract`, but its
deploy and initialize steps are outdated:

- they call the deprecated `soroban` CLI
- `initialize` now requires `--admin`
- the verification key parameter is `vk` (a struct), not `--verification_key`
- the model commitment and key fall back to placeholder values

Until the script is updated, follow
[docs/guides/deployment.md](../../docs/guides/deployment.md).

## 4. Run the demo (pending)

```bash
cargo run -p zkml-demo -- --model kyc_decision_tree.onnx --contract-id <CONTRACT_ID>
```

The runner currently prints the planned steps only.

## Mapping the result

For decision trees, `class_label` is always `0` and `output` carries the leaf value
(raw Q16.16). That equals the risk tier only for JSON trees whose leaves store the
tier (tier 2 is `131072`). Multi-class ONNX trees such as `kyc_decision_tree.onnx`
are not mapped to a tier yet.

## Success criteria

- Proof size under 500 bytes (Groth16 is 256 bytes in the contract encoding)
- End-to-end latency under 60 seconds
- Verified risk tier readable on-chain

## Features

| Feature | Description |
| ------- | ----------- |
| `age` | Age in years (18 to 80) |
| `account_age_days` | Account age in days |
| `transaction_count_30d` | Transactions in the last 30 days |
| `avg_transaction_amount` | Average transaction amount |
| `has_verified_doc` | Document verification status (0/1) |
| `jurisdiction_risk_score` | Jurisdiction risk score (0 to 100) |
| `login_frequency_30d` | Logins in the last 30 days |
| `device_trust_score` | Device trust score (0 to 100) |
| `email_domain_age_days` | Email domain age in days |
| `phone_verified` | Phone verification status (0/1) |

## Next steps

1. STARK to Groth16 compression in `zkml-prover`
2. Verification key export
3. Contract path for RISC Zero Groth16 public inputs
4. Update `train_model.py` opset and `deploy.sh` to the `stellar` CLI
5. Implement the `zkml-demo` pipeline with metrics

## References

- [Use cases: provable KYC risk scoring](../../docs/project/use-cases.md)
- [Roadmap](../../docs/project/roadmap.md)
- [CAP-0074: BN254 host functions](https://stellar.org/protocol/cap-0074)
- [CAP-0075: Poseidon hash functions](https://stellar.org/protocol/cap-0075)
