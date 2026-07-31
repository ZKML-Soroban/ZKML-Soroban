# KYC Demo - End-to-End zkml-soroban Example

This directory contains a complete end-to-end demonstration of zkml-soroban for KYC risk scoring, fulfilling Roadmap Milestone 1.9.

## Overview

This demo shows how to:
1. Train a decision tree model on synthetic KYC data
2. Export the model to ONNX and zkml-soroban JSON formats
3. Deploy the verifier contract to Stellar testnet
4. Run inference on sample user data
5. Generate a zero-knowledge proof
6. Submit and verify the proof on-chain
7. Measure performance against success criteria

## Use Case

This implements **Use Case 1: Provable KYC Risk Scoring** from `docs/use-cases.md`:
- **Model**: Decision tree classifier
- **Features**: 10 KYC-related features (age, account age, transaction history, verification scores, etc.)
- **Output**: Risk tier (0 = low, 1 = medium, 2 = high)
- **Goal**: Enable trust-minimized compliance across Stellar anchors

## Prerequisites

### 1. Python Environment (for model training)

Install Python 3.9+ and required packages:

```bash
pip install -r requirements.txt
```

Required packages:
- numpy >= 1.24.0
- pandas >= 2.0.0
- scikit-learn >= 1.3.0
- skl2onnx >= 1.14.0
- onnx >= 1.15.0

### 2. Rust Toolchain (for prover and demo)

Install Rust from https://rustup.rs:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 3. Stellar CLI (for contract deployment)

Install Stellar CLI from https://github.com/stellar/stellar-cli:

```bash
cargo install stellar-cli
```

Or download the binary for your platform.

### 4. Soroban CLI (for contract interaction)

Install Soroban CLI from https://github.com/stellar/soroban-cli:

```bash
cargo install soroban-cli
```

### 5. Testnet Account

Create a Stellar identity and fund it from friendbot:

```bash
stellar identity create
stellar friendbot fund $(stellar identity address)
```

## Quick Start

### Step 1: Generate and Train Model

Generate synthetic KYC data and train the decision tree model:

```bash
python train_export.py
```

This creates:
- `kyc_dataset.csv` - Synthetic training data (1000 samples)
- `kyc_decision_tree.onnx` - Model in ONNX format
- `kyc_decision_tree.json` - Model in zkml-soroban JSON format

**Expected output:**
```
Generating synthetic KYC dataset...
Saved dataset to kyc_dataset.csv
Dataset shape: (1000, 11)
Risk tier distribution:
0    450
1    350
2    200

Training decision tree model...
Model Evaluation:
              precision    recall  f1-score   support
           0       0.92      0.95      0.93        90
           1       0.88      0.83      0.85        70
           2       0.90      0.90      0.90        40

Exporting to ONNX format...
Saved ONNX model to kyc_decision_tree.onnx
Converting to zkml-soroban JSON format...
Saved JSON model to kyc_decision_tree.json

✓ Training and export complete!
  - Dataset: kyc_dataset.csv
  - ONNX model: kyc_decision_tree.onnx
  - JSON model: kyc_decision_tree.json
  - Features: 10
  - Tree nodes: 23
```

### Step 2: Deploy Verifier Contract to Testnet

Deploy the zkml-verifier contract to Stellar testnet:

**On Windows (PowerShell):**
```powershell
./deploy.ps1 -ModelPath kyc_decision_tree.json -Network testnet
```

**On Linux/macOS:**
```bash
./deploy.sh --model-path kyc_decision_tree.json --network testnet
```

**Expected output:**
```
==========================================
  zkml-verifier Testnet Deployment
==========================================

Checking prerequisites...
✓ Stellar CLI found: stellar-cli 21.0.0
✓ Soroban CLI found: soroban-cli 22.0.0
✓ Rust found: rustc 1.75.0
✓ Model file found: kyc_decision_tree.json

Building verifier contract...
✓ Contract built successfully

Checking testnet account...
✓ Using identity: GABCD...
  Balance: 10000.00 XLM

Deploying contract to testnet...
✓ Contract deployed: CDEFG...

Calculating model commitment...
✓ Model commitment: a1b2c3d4...

Initializing contract...
✓ Contract initialized with model commitment

==========================================
  Deployment Complete!
==========================================

Contract ID: CDEFG...
Model Commitment: a1b2c3d4...
```

**Save the contract ID** for the next step:
```bash
export CONTRACT_ID=CDEFG...
```

### Step 3: Run End-to-End Demo

Run the demo with the deployed contract:

```bash
cargo run -p zkml-demo -- --model kyc_decision_tree.json --contract $CONTRACT_ID
```

Or in local mode (without contract submission):
```bash
cargo run -p zkml-demo -- --model kyc_decision_tree.json --local
```

**Expected output:**
```
=== zkml-soroban KYC Demo ===

Step 1: Importing model...
✓ Model imported in 15 ms
  Model type: DecisionTree
  Num features: 10

Step 2: Calculating model commitment...
✓ Model commitment: a1b2c3d4...

Step 3: Processing input features...
✓ Processed 10 features
  Feature 0: 35.0 (quantized)
  Feature 1: 180.0 (quantized)
  ...

Step 4: Running inference...
✓ Inference completed in 2 ms
  Output (raw): 1
  Output (dequantized): 1.0
  Risk tier: 1 (Medium)

Step 5: Generating ZK proof...
✓ Proof generated in 4500 ms
  Proof size: 128 bytes
  Public inputs size: 72 bytes
  Total bundle size: 200 bytes

Step 6: Submitting to contract...
✓ Contract submission completed in 12000 ms

Step 7: Querying verified result...
✓ Verified result retrieved
  Model hash: a1b2c3d4...
  Output: [1]
  Verified at: 12345

=== Timing Metrics ===
Model import:      15 ms
Inference:          2 ms
Proof generation:   4500 ms
Contract submission: 12000 ms
Total:              16517 ms

=== Success Criteria Check ===
Proof size < 500 bytes: ✓ PASS (128 bytes)
End-to-end latency < 60s: ✓ PASS (16517 ms)

Overall: ✓ ALL CRITERIA PASS
```

## Custom Input Features

You can provide custom user features:

```bash
cargo run -p zkml-demo -- \
  --model kyc_decision_tree.json \
  --contract $CONTRACT_ID \
  --features "25,30,10,50,0.8,1,1,0.1,0.2,0.9"
```

Feature order (10 features):
1. `age` - User age (18-80)
2. `account_age_days` - Days since account creation
3. `transaction_count` - Number of transactions
4. `avg_transaction_amount` - Average transaction value
5. `document_verification_score` - Document verification (0-1)
6. `email_verified` - Binary (0/1)
7. `phone_verified` - Binary (0/1)
8. `jurisdiction_risk` - Jurisdiction risk score (0-1)
9. `ip_risk_score` - IP-based risk score (0-1)
10. `device_trust_score` - Device trust score (0-1)

## Success Criteria

The demo validates the Phase 1 exit criteria:

| Criterion | Target | Status |
|-----------|--------|--------|
| Decision tree inference | ✓ Working | Implemented |
| Groth16 proof verification | ✓ Working | Implemented (placeholder) |
| End-to-end latency | < 60 seconds | Measured |
| Proof size | < 500 bytes | Measured |

**Note:** The current implementation uses a placeholder Groth16 proof. Real cryptographic verification requires completion of:
- Issue #11: STARK-to-Groth16 wrapping
- Issue #12: BN254 host functions integration
- Issue #13: Poseidon commitments

## Troubleshooting

### Model Training Fails

If Python packages fail to install:
```bash
python -m pip install --upgrade pip
pip install -r requirements.txt
```

### Contract Deployment Fails

If friendbot funding fails:
```bash
stellar friendbot fund $(stellar identity address)
```

If contract build fails:
```bash
cargo build --release --package zkml-verifier --target wasm32-unknown-unknown
```

### Demo Runner Fails

If the demo fails to find the model:
```bash
# Ensure you're in the kyc-demo directory
cd examples/kyc-demo
cargo run -p zkml-demo -- --model kyc_decision_tree.json --local
```

If contract submission fails, use local mode:
```bash
cargo run -p zkml-demo -- --model kyc_decision_tree.json --local
```

## Architecture

The demo follows the zkml-soroban architecture:

```
┌─────────────────┐
│  Model Training │  (Python + scikit-learn)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  ONNX Export    │  (skl2onnx)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  JSON Convert   │  (zkml-soroban format)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Contract Deploy│  (Stellar CLI + Soroban)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Demo Runner    │  (zkml-demo CLI)
│  - Import       │
│  - Quantize     │
│  - Inference    │
│  - Prove        │
│  - Submit       │
│  - Verify       │
└─────────────────┘
```

## Next Steps

After running this demo:

1. **Explore the code**: Read the source in `crates/zkml-demo/src/main.rs`
2. **Modify the model**: Edit `train_export.py` to change features or tree depth
3. **Integrate with your app**: Use the zkml-prover library in your own application
4. **Read the docs**: See `docs/` for architecture, API reference, and more

## References

- [Roadmap Milestone 1.9](../../docs/roadmap.md#milestone-19)
- [Use Case 1: KYC Risk Scoring](../../docs/use-cases.md#use-case-1-provable-kyc-risk-scoring)
- [Stellar CAP-0074: BN254 Host Functions](https://stellar.org/protocol/cap-0074)
- [Stellar CAP-0075: Poseidon Hash Functions](https://stellar.org/protocol/cap-0075)
