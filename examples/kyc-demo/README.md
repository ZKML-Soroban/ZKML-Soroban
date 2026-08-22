# KYC Demo - End-to-End Provable ML Inference on Stellar Testnet

This demo demonstrates end-to-end provable KYC risk scoring using zkml-soroban on Stellar testnet.

## Overview

The demo trains a decision tree model on synthetic KYC data, deploys a verifier contract to testnet, and runs inference with proof generation and verification.

## Prerequisites

### 1. Stellar Testnet Account

You need a Stellar testnet account with XLM for deployment and transaction fees:

```bash
# Install stellar-cli
# See: https://developers.stellar.org/docs/start/software/cli

# Create a new testnet account
stellar keys generate testnet_account

# Fund via friendbot
stellar friendbot fund <your_public_key>
```

### 2. Soroban CLI

```bash
# Install soroban-cli
cargo install soroban-cli
```

### 3. Python Dependencies

```bash
pip install numpy pandas scikit-learn skl2onnx
```

### 4. Rust Toolchain

```bash
# Install Rust if not already installed
# See: https://rustup.rs/

# Add wasm32 target
rustup target add wasm32-unknown-unknown
```

## Quick Start

### 1. Generate Synthetic KYC Dataset

```bash
cd examples/kyc-demo
python generate_dataset.py
```

This creates `kyc_dataset.csv` with 1000 synthetic KYC records.

### 2. Train Decision Tree Model

```bash
python train_model.py
```

This trains a decision tree classifier and exports it to `kyc_decision_tree.onnx`.

### 3. Deploy Verifier Contract

```bash
export DEPLOYER_PUBLIC_KEY=<your_public_key>
export DEPLOYER_SECRET_KEY=<your_secret_key>
bash deploy.sh
```

This builds the verifier WASM, deploys it to testnet, and initializes it with the model commitment.

**Note:** The deployment script currently uses placeholder values for:
- Model commitment (Poseidon hash of model parameters)
- Verification key (Groth16 verification key)

These require implementation of:
- Poseidon commitment calculation (CAP-0075)
- Verification key extraction from risc0-groth16 3.0.5

### 4. Run Demo (Not Yet Implemented)

```bash
# TODO: Create zkml-demo crate
cargo run -p zkml-demo -- --model kyc_decision_tree.onnx --contract-id <CONTRACT_ID>
```

## Current Status

### ✅ Completed

- Synthetic KYC dataset generation
- Decision tree training and ONNX export
- Deployment script scaffolding
- Infrastructure for demo runner

### ⏳ Pending Dependencies

The end-to-end demo depends on the following features that are not yet implemented:

1. **STARK→Groth16 Compression** (Milestone 1.7)
   - Currently returns `Err("not yet implemented")`
   - Requires investigation of risc0-groth16 3.0.5 API
   - See: `crates/zkml-prover/src/prover.rs`

2. **Verification Key Export** (Milestone 1.7)
   - Currently stubbed with TODO comment
   - Requires risc0-groth16 VK extraction API
   - See: `crates/zkml-prover/src/prover.rs`

3. **Poseidon Commitments** (Milestone 1.8)
   - Infrastructure exists in zkml-common
   - Integration with model parameters needed
   - See: `crates/zkml-common/src/commitment.rs`

4. **Real Groth16 Verification** (Milestone 1.8)
   - Verifier contract structure exists
   - BN254 pairing check implementation needed
   - See: `crates/zkml-verifier/src/lib.rs`

## Success Criteria

Once dependencies are implemented, the demo should achieve:

- **Proof size < 500 bytes** (Groth16 proof wire format)
- **End-to-end latency < 60 seconds** (proof generation + submission + verification)
- **Verified risk tier output** (0=low, 1=medium, 2=high)

## Development Notes

### Dataset Features

The synthetic KYC dataset includes 10 features:
- `age`: Age in years (18-80)
- `account_age_days`: Account age in days
- `transaction_count_30d`: Transaction count last 30 days
- `avg_transaction_amount`: Average transaction amount
- `has_verified_doc`: Document verification status (0/1)
- `jurisdiction_risk_score`: Jurisdiction risk score (0-100)
- `login_frequency_30d`: Login frequency last 30 days
- `device_trust_score`: Device trust score (0-100)
- `email_domain_age_days`: Email domain age in days
- `phone_verified`: Phone verification status (0/1)

### Model Architecture

- **Type**: Decision tree classifier
- **Max depth**: 5 (for interpretability and smaller circuit size)
- **Output**: Risk tier (0=low, 1=medium, 2=high)
- **Format**: ONNX with opset 12

### Next Steps

1. Implement STARK→Groth16 compression in `zkml-prover`
2. Implement verification key extraction
3. Implement Poseidon commitment for model parameters
4. Implement real BN254 pairing check in verifier
5. Create `zkml-demo` crate for demo runner CLI
6. Add metrics output and success criteria checks
7. Validate README walkthrough with another contributor

## References

- [Roadmap Milestone 1.9](../../docs/roadmap.md#milestone-19-testnet-deployment)
- [Use Case 1: Provable KYC Risk Scoring](../../docs/use-cases.md#use-case-1-provable-kyc-risk-scoring)
- [Stellar CLI Setup](https://developers.stellar.org/docs/start/software/cli)
- [CAP-0074: BN254 Host Functions](https://stellar.org/protocol/cap-0074)
- [CAP-0075: Poseidon Hash Functions](https://stellar.org/protocol/cap-0075)
