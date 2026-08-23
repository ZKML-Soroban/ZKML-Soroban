#!/usr/bin/env python3
"""
Train a decision tree model on synthetic KYC data and export to ONNX.

This script trains a decision tree classifier for KYC risk scoring and exports
it to ONNX format for use with zkml-soroban.
"""

import pandas as pd
import numpy as np
from pathlib import Path
from sklearn.tree import DecisionTreeClassifier
from sklearn.model_selection import train_test_split
from sklearn.metrics import classification_report, accuracy_score
import skl2onnx
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType

# Feature names (must match generate_dataset.py)
FEATURES = [
    "age",
    "account_age_days",
    "transaction_count_30d",
    "avg_transaction_amount",
    "has_verified_doc",
    "jurisdiction_risk_score",
    "login_frequency_30d",
    "device_trust_score",
    "email_domain_age_days",
    "phone_verified",
]

def load_dataset(csv_path):
    """Load the synthetic KYC dataset."""
    df = pd.read_csv(csv_path)
    X = df[FEATURES].values.astype(np.float32)
    y = df["risk_tier"].values.astype(np.int64)
    return X, y

def train_decision_tree(X_train, y_train, max_depth=5):
    """Train a decision tree classifier."""
    # Use a shallow tree for interpretability and smaller circuit size
    model = DecisionTreeClassifier(
        max_depth=max_depth,
        min_samples_split=10,
        min_samples_leaf=5,
        random_state=42
    )
    model.fit(X_train, y_train)
    return model

def evaluate_model(model, X_test, y_test):
    """Evaluate the trained model."""
    y_pred = model.predict(X_test)
    accuracy = accuracy_score(y_test, y_pred)
    print(f"Model accuracy: {accuracy:.3f}")
    print("\nClassification report:")
    print(classification_report(y_test, y_pred, target_names=["low", "medium", "high"]))
    return accuracy

def export_to_onnx(model, output_path):
    """Export the sklearn model to ONNX format."""
    # Define input shape (batch_size=1, num_features=len(FEATURES))
    initial_type = [("float_input", FloatTensorType([None, len(FEATURES)]))]
    
    # Convert to ONNX
    onnx_model = convert_sklearn(
        model,
        initial_types=initial_type,
        target_opset=12,  # Use opset 12 for compatibility
        zipmap=False  # Disable class probability outputs
    )
    
    # Save the ONNX model
    with open(output_path, "wb") as f:
        f.write(onnx_model.SerializeToString())
    
    print(f"Exported ONNX model to: {output_path}")

def main():
    """Main training pipeline."""
    output_dir = Path(__file__).parent
    dataset_path = output_dir / "kyc_dataset.csv"
    onnx_output_path = output_dir / "kyc_decision_tree.onnx"
    
    # Load dataset
    print("Loading dataset...")
    X, y = load_dataset(dataset_path)
    print(f"Dataset shape: {X.shape}")
    
    # Split into train/test
    X_train, X_test, y_train, y_test = train_test_split(
        X, y, test_size=0.2, random_state=42, stratify=y
    )
    print(f"Training samples: {len(X_train)}, Test samples: {len(X_test)}")
    
    # Train model
    print("\nTraining decision tree...")
    model = train_decision_tree(X_train, y_train, max_depth=5)
    
    # Evaluate
    print("\nEvaluating model...")
    evaluate_model(model, X_test, y_test)
    
    # Export to ONNX
    print("\nExporting to ONNX...")
    export_to_onnx(model, onnx_output_path)
    
    print("\nTraining complete!")
    print(f"ONNX model saved to: {onnx_output_path}")
    print("\nNext steps:")
    print("1. Import the ONNX model using zkml-prover")
    print("2. Quantize the model for fixed-point inference")
    print("3. Generate proofs using the prover")

if __name__ == "__main__":
    main()
