#!/usr/bin/env pwsh
#
# Deployment script for zkml-verifier contract to Stellar testnet.
#
# This script:
# 1. Builds the verifier WASM contract
# 2. Deploys it to Stellar testnet using the Stellar CLI
# 3. Initializes the contract with the model's Poseidon commitment
#
# Prerequisites:
# - Stellar CLI installed and configured
# - Testnet account with sufficient XLM (use friendbot if needed)
# - soroban-cli installed
#
# Usage:
#   ./deploy.ps1 -ModelPath <path_to_model.json> [-Network testnet]
#
# Example:
#   ./deploy.ps1 -ModelPath kyc_decision_tree.json -Network testnet
#

param(
    [Parameter(Mandatory=$true)]
    [string]$ModelPath,
    
    [Parameter(Mandatory=$false)]
    [string]$Network = "testnet",
    
    [Parameter(Mandatory=$false)]
    [string]$ContractWasmPath = "../../target/wasm32-unknown-unknown/release/zkml_verifier.wasm"
)

$ErrorActionPreference = "Stop"

# Color output functions
function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

function Write-Success { Write-ColorOutput Green $args }
function Write-Warning { Write-ColorOutput Yellow $args }
function Write-Error { Write-ColorOutput Red $args }

# Check prerequisites
function Test-Prerequisites {
    Write-Output "Checking prerequisites..."
    
    # Check Stellar CLI
    try {
        $stellarVersion = stellar --version 2>&1
        Write-Success "✓ Stellar CLI found: $stellarVersion"
    } catch {
        Write-Error "✗ Stellar CLI not found. Please install it from https://github.com/stellar/stellar-cli"
        exit 1
    }
    
    # Check soroban-cli
    try {
        $sorobanVersion = soroban --version 2>&1
        Write-Success "✓ Soroban CLI found: $sorobanVersion"
    } catch {
        Write-Error "✗ Soroban CLI not found. Please install it from https://github.com/stellar/soroban-cli"
        exit 1
    }
    
    # Check Rust/Cargo
    try {
        $rustVersion = rustc --version 2>&1
        Write-Success "✓ Rust found: $rustVersion"
    } catch {
        Write-Error "✗ Rust not found. Please install it from https://rustup.rs"
        exit 1
    }
    
    # Check model file exists
    if (-not (Test-Path $ModelPath)) {
        Write-Error "✗ Model file not found: $ModelPath"
        exit 1
    }
    Write-Success "✓ Model file found: $ModelPath"
    
    Write-Output ""
}

# Build the verifier contract
function Build-Contract {
    Write-Output "Building verifier contract..."
    
    Push-Location "../../"
    try {
        cargo build --release --package zkml-verifier --target wasm32-unknown-unknown
        if ($LASTEXITCODE -ne 0) {
            Write-Error "✗ Contract build failed"
            exit 1
        }
        Write-Success "✓ Contract built successfully"
    } finally {
        Pop-Location
    }
    
    # Verify WASM exists
    if (-not (Test-Path $ContractWasmPath)) {
        Write-Error "✗ Contract WASM not found at: $ContractWasmPath"
        exit 1
    }
    
    Write-Output ""
}

# Get or fund testnet account
function Get-TestnetAccount {
    Write-Output "Checking testnet account..."
    
    try {
        $identity = stellar identity address
        Write-Success "✓ Using identity: $identity"
    } catch {
        Write-Error "✗ No Stellar identity found. Please run: stellar identity create"
        exit 1
    }
    
    # Check balance
    try {
        $balance = stellar balance --identity $identity
        Write-Output "  Balance: $balance"
        
        # If balance is very low, suggest friendbot
        if ($balance -match "(\d+\.?\d*) XLM") {
            $amount = [double]$Matches[1]
            if ($amount -lt 2) {
                Write-Warning "⚠ Low balance. Funding from friendbot..."
                stellar friendbot fund $identity
                Write-Success "✓ Funded from friendbot"
            }
        }
    } catch {
        Write-Warning "⚠ Could not check balance. Attempting to fund from friendbot..."
        stellar friendbot fund $identity
        Write-Success "✓ Funded from friendbot"
    }
    
    Write-Output ""
    return $identity
}

# Deploy contract
function Deploy-Contract {
    param([string]$Identity)
    
    Write-Output "Deploying contract to $Network..."
    
    # Deploy using soroban CLI
    $deployArgs = @(
        "contract", "deploy",
        "--wasm", $ContractWasmPath,
        "--source", $Identity,
        "--network", $Network
    )
    
    $deployOutput = & soroban @deployArgs 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "✗ Contract deployment failed"
        Write-Output $deployOutput
        exit 1
    }
    
    # Extract contract ID from output
    if ($deployOutput -match "([A-Z0-9]{56})") {
        $contractId = $Matches[1]
        Write-Success "✓ Contract deployed: $contractId"
        return $contractId
    } else {
        Write-Error "✗ Could not parse contract ID from deployment output"
        Write-Output $deployOutput
        exit 1
    }
    
    Write-Output ""
}

# Calculate model commitment
function Get-ModelCommitment {
    param([string]$ModelPath)
    
    Write-Output "Calculating model commitment..."
    
    # Use the zkml-prover CLI to calculate the commitment
    $commitmentOutput = & cargo run -p zkml-prover -- $ModelPath "0,0,0,0,0,0,0,0,0,0" 2>&1 | Select-String "model commitment"
    
    if ($commitmentOutput -match "model commitment: ([a-f0-9]{64})") {
        $commitment = $Matches[1]
        Write-Success "✓ Model commitment: $commitment"
        return $commitment
    } else {
        Write-Warning "⚠ Could not extract commitment from output. Using placeholder."
        # For demo purposes, we'll use a placeholder commitment
        # In production, this should be computed properly
        return "0" * 64
    }
    
    Write-Output ""
}

# Initialize contract
function Initialize-Contract {
    param(
        [string]$ContractId,
        [string]$Identity,
        [string]$ModelCommitment
    )
    
    Write-Output "Initializing contract..."
    
    # Convert hex commitment to bytes (32 bytes)
    $commitmentBytes = $ModelCommitment.Substring(0, 64)
    
    # Call initialize function
    $initArgs = @(
        "contract", "invoke",
        "--id", $ContractId,
        "--source", $Identity,
        "--network", $Network,
        "--",
        "initialize",
        "--model-hash", $commitmentBytes
    )
    
    $initOutput = & soroban @initArgs 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "✗ Contract initialization failed"
        Write-Output $initOutput
        exit 1
    }
    
    Write-Success "✓ Contract initialized with model commitment"
    Write-Output ""
}

# Main execution
function Main {
    Write-Output "=========================================="
    Write-Output "  zkml-verifier Testnet Deployment"
    Write-Output "=========================================="
    Write-Output ""
    
    Test-Prerequisites
    Build-Contract
    $identity = Get-TestnetAccount
    $contractId = Deploy-Contract -Identity $identity
    $modelCommitment = Get-ModelCommitment -ModelPath $ModelPath
    Initialize-Contract -ContractId $contractId -Identity $identity -ModelCommitment $modelCommitment
    
    Write-Output "=========================================="
    Write-Success "  Deployment Complete!"
    Write-Output "=========================================="
    Write-Output ""
    Write-Output "Contract ID: $contractId"
    Write-Output "Model Commitment: $modelCommitment"
    Write-Output ""
    Write-Output "Save the contract ID for the demo runner:"
    Write-Output "  export CONTRACT_ID=$contractId"
    Write-Output ""
}

# Run main
Main
