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
| Training and ONNX export (`train_model.py`) | Works, exports with `target_opset=17` |
| ONNX import into `zkml-prover` | Works: a decision tree of 43 nodes over 10 features |
| Verifier contract with Groth16 (BN254) verification | Implemented |
| Poseidon model commitment | Implemented (`zkml-prover commit` for JSON models) |
| Contract build and deployment | Works with the `stellar` CLI (`deploy.sh`) |
| STARK to Groth16 compression | Works on x86_64 Linux with Docker (`zkml-prover prove --groth16`) |
| RISC Zero parameter export | Works (`zkml-prover export-vk --format soroban-args`) |
| On-chain verification (`verify_receipt`) | The KYC tree verified on testnet on 2026-11-20, see the deployment guide |
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

The export uses `target_opset=17`, the floor the importer enforces for the core
domain. To check the file before importing it:

```bash
python check_model.py
```

It prints every opset import and every operator, and exits non-zero when the
model would be rejected.

## 3. Deploy, prove and verify on testnet

From the repository root:

```bash
examples/kyc-demo/deploy.sh
```

The script creates and funds the `zkml-kyc-demo` identity if it does not exist,
generates the model's secret opening in `target/kyc-demo/model.opening` (keep
it: every later proof for this contract needs it), deploys the verifier with the KYC tree's commitment and no native key, registers
the reproducible guest (`set_risc0_config`, `set_risc0_vk`), proves one risk
score with Groth16, submits it with `verify_receipt` and reads the result back.
Each step prints its command, and the stellar CLI prints a link to each
transaction.

Proving and exporting need x86_64 Linux with Docker; proving takes about 25
minutes of CPU. On another machine, make the bundle and the RISC Zero values on
such a host and pass them in:

```bash
examples/kyc-demo/deploy.sh --bundle bundle.json --risc0-params risc0_params.json
```

`--help` lists every option. The model and input default to
`examples/models/kyc_tree.json` and `0.6,0.4,0.0`; `KYC_MODEL` and `KYC_INPUT`
change them. The CLI reads JSON models only, so the ONNX model from step 2 has
to be converted first.

## 4. Run the demo runner (pending)

```bash
cargo run -p zkml-demo -- --model kyc_decision_tree.onnx --contract-id <CONTRACT_ID>
```

The runner currently prints the planned steps only; `deploy.sh` is the working
path.

## Mapping the result

For decision trees, `class_label` is always `0` and `output` carries the leaf value
(raw Q16.16). For JSON trees whose leaves store the tier, that is the tier (tier 2
is `131072`). Multi-class ONNX trees such as `kyc_decision_tree.onnx` import with
the predicted class in each leaf, so their output is the tier too.

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

1. Accept ONNX models in the CLI, so the trained model can be proved directly
2. Implement the `zkml-demo` pipeline with metrics
3. Remote proving, to bring the 25-minute proof under the latency target

## References

- [Use cases: provable KYC risk scoring](../../docs/project/use-cases.md)
- [Roadmap](../../docs/project/roadmap.md)
- [CAP-0074: BN254 host functions](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0074.md)
- [CAP-0075: Poseidon hash functions](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0075.md)
