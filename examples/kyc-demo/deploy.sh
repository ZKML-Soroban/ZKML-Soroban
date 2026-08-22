#!/usr/bin/env bash
set -e

# Deployment script for zkml-verifier contract to Stellar testnet
#
# This script:
# 1. Builds the verifier WASM
# 2. Deploys it to testnet via stellar CLI
# 3. Calls initialize with the model's Poseidon commitment and verification key
#
# Prerequisites:
# - stellar-cli installed and configured
# - soroban-cli installed
# - testnet account with sufficient XLM
# - friendbot funding for testnet account
#
# TODO: This script is a scaffold. The following features are not yet implemented:
# - Actual Groth16 verification key extraction (risc0-groth16 3.0.5 API investigation needed)
# - Poseidon commitment calculation for model parameters
# - Real verification key format for contract initialization

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERIFIER_WASM="$PROJECT_ROOT/target/wasm32-unknown-unknown/release/zkml_verifier.wasm"

# Network configuration
NETWORK="testnet"
SOROBAN_RPC_URL="https://soroban-testnet.stellar.org:443"
SOROBAN_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

# Account configuration (set via environment variables)
DEPLOYER_PUBLIC_KEY="${DEPLOYER_PUBLIC_KEY:-}"
DEPLOYER_SECRET_KEY="${DEPLOYER_SECRET_KEY:-}"

# Model configuration
MODEL_ONNX_PATH="$SCRIPT_DIR/kyc_decision_tree.onnx"
MODEL_COMMITMENT_HEX="${MODEL_COMMITMENT_HEX:-}"
VERIFICATION_KEY_HEX="${VERIFICATION_KEY_HEX:-}"

echo "=== zkml-verifier Deployment Script ==="
echo "Network: $NETWORK"
echo "RPC URL: $SOROBAN_RPC_URL"
echo ""

# Check prerequisites
if [ -z "$DEPLOYER_PUBLIC_KEY" ] || [ -z "$DEPLOYER_SECRET_KEY" ]; then
    echo "Error: DEPLOYER_PUBLIC_KEY and DEPLOYER_SECRET_KEY must be set"
    echo "Example: export DEPLOYER_PUBLIC_KEY=GB..."
    echo "         export DEPLOYER_SECRET_KEY=S..."
    exit 1
fi

if [ ! -f "$MODEL_ONNX_PATH" ]; then
    echo "Error: Model file not found: $MODEL_ONNX_PATH"
    echo "Run train_model.py first to generate the model"
    exit 1
fi

# Build the verifier WASM
echo "Building verifier WASM..."
cd "$PROJECT_ROOT"
cargo build --release --target wasm32-unknown-unknown -p zkml-verifier

if [ ! -f "$VERIFIER_WASM" ]; then
    echo "Error: WASM build failed: $VERIFIER_WASM not found"
    exit 1
fi

echo "WASM built successfully: $VERIFIER_WASM"
echo ""

# Calculate model commitment (TODO: implement Poseidon commitment)
if [ -z "$MODEL_COMMITMENT_HEX" ]; then
    echo "Warning: MODEL_COMMITMENT_HEX not set"
    echo "TODO: Implement Poseidon commitment calculation for model parameters"
    echo "For now, using placeholder commitment"
    MODEL_COMMITMENT_HEX="0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
fi

# Get verification key (TODO: implement VK extraction)
if [ -z "$VERIFICATION_KEY_HEX" ]; then
    echo "Warning: VERIFICATION_KEY_HEX not set"
    echo "TODO: Implement verification key extraction from risc0-groth16"
    echo "For now, using placeholder VK"
    VERIFICATION_KEY_HEX="0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
fi

# Deploy contract
echo "Deploying contract to testnet..."
CONTRACT_ID=$(soroban contract deploy \
    --wasm "$VERIFIER_WASM" \
    --source "$DEPLOYER_SECRET_KEY" \
    --rpc-url "$SOROBAN_RPC_URL" \
    --network-passphrase "$SOROBAN_NETWORK_PASSPHRASE" \
    --fee 10000)

echo "Contract deployed: $CONTRACT_ID"
echo ""

# Initialize contract
echo "Initializing contract with model commitment and verification key..."
soroban contract invoke \
    --id "$CONTRACT_ID" \
    --source "$DEPLOYER_SECRET_KEY" \
    --rpc-url "$SOROBAN_RPC_URL" \
    --network-passphrase "$SOROBAN_NETWORK_PASSPHRASE" \
    --fee 10000 \
    -- \
    initialize \
    --model_hash "$MODEL_COMMITMENT_HEX" \
    --verification_key "$VERIFICATION_KEY_HEX"

echo ""
echo "=== Deployment Complete ==="
echo "Contract ID: $CONTRACT_ID"
echo "Model commitment: $MODEL_COMMITMENT_HEX"
echo ""
echo "Save the CONTRACT_ID for the demo runner:"
echo "export CONTRACT_ID=$CONTRACT_ID"
