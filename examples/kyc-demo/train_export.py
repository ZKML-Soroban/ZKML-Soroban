#!/usr/bin/env python3
"""
Generate synthetic KYC dataset and train/export a decision tree model to ONNX.

This script creates a synthetic KYC dataset with 5-15 features representing
typical KYC assessment factors, trains a decision tree classifier, and exports
the model to ONNX format using skl2onnx.

Usage:
    python train_export.py

Outputs:
    - kyc_dataset.csv: Synthetic training data (no real PII)
    - kyc_decision_tree.onnx: Trained model in ONNX format
    - kyc_decision_tree.json: Model in zkml-soroban JSON format
"""

import numpy as np
import pandas as pd
from sklearn.tree import DecisionTreeClassifier
from sklearn.model_selection import train_test_split
from sklearn.metrics import classification_report
import skl2onnx
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType
import json
import sys

# Set random seed for reproducibility
np.random.seed(42)

def generate_synthetic_kyc_data(n_samples=1000):
    """
    Generate synthetic KYC dataset with realistic features.
    
    Features (5-15 as per use case):
    1. age: User age (18-80)
    2. account_age_days: Days since account creation
    3. transaction_count: Number of transactions
    4. avg_transaction_amount: Average transaction value
    5. document_verification_score: Document verification (0-1)
    6. email_verified: Binary (0/1)
    7. phone_verified: Binary (0/1)
    8. jurisdiction_risk: Jurisdiction risk score (0-1)
    9. ip_risk_score: IP-based risk score (0-1)
    10. device_trust_score: Device trust score (0-1)
    
    Output: risk_tier (0=low, 1=medium, 2=high)
    """
    data = []
    
    for _ in range(n_samples):
        # Generate features with realistic distributions
        age = np.random.randint(18, 80)
        account_age_days = np.random.randint(0, 3650)  # 0-10 years
        transaction_count = np.random.randint(0, 500)
        avg_transaction_amount = np.random.exponential(100)  # Skewed distribution
        document_verification_score = np.random.beta(2, 1)  # Mostly high scores
        email_verified = np.random.choice([0, 1], p=[0.1, 0.9])
        phone_verified = np.random.choice([0, 1], p=[0.15, 0.85])
        jurisdiction_risk = np.random.beta(1, 3)  # Mostly low risk
        ip_risk_score = np.random.beta(1, 2)
        device_trust_score = np.random.beta(3, 1)
        
        # Calculate risk tier based on feature combinations
        # Low risk (0): Good verification, low jurisdiction risk, established account
        # Medium risk (1): Mixed signals
        # High risk (2): Poor verification, high risk scores, new account
        
        risk_score = (
            (1 - document_verification_score) * 0.3 +
            jurisdiction_risk * 0.25 +
            ip_risk_score * 0.2 +
            (1 - device_trust_score) * 0.15 +
            (1 - email_verified) * 0.05 +
            (1 - phone_verified) * 0.05
        )
        
        # Adjust for account age (new accounts are riskier)
        if account_age_days < 30:
            risk_score += 0.2
        
        # Determine risk tier
        if risk_score < 0.3:
            risk_tier = 0
        elif risk_score < 0.6:
            risk_tier = 1
        else:
            risk_tier = 2
        
        row = {
            'age': age,
            'account_age_days': account_age_days,
            'transaction_count': transaction_count,
            'avg_transaction_amount': avg_transaction_amount,
            'document_verification_score': document_verification_score,
            'email_verified': email_verified,
            'phone_verified': phone_verified,
            'jurisdiction_risk': jurisdiction_risk,
            'ip_risk_score': ip_risk_score,
            'device_trust_score': device_trust_score,
            'risk_tier': risk_tier
        }
        data.append(row)
    
    return pd.DataFrame(data)

def train_model(df):
    """Train a decision tree classifier on the synthetic data."""
    X = df.drop('risk_tier', axis=1)
    y = df['risk_tier']
    
    # Split for evaluation
    X_train, X_test, y_train, y_test = train_test_split(
        X, y, test_size=0.2, random_state=42, stratify=y
    )
    
    # Train decision tree (max depth to keep it simple for demo)
    clf = DecisionTreeClassifier(
        max_depth=5,
        min_samples_split=10,
        random_state=42
    )
    clf.fit(X_train, y_train)
    
    # Print evaluation
    print("Model Evaluation:")
    print(classification_report(y_test, clf.predict(X_test)))
    
    return clf

def export_to_onnx(model, feature_names):
    """Export sklearn model to ONNX format."""
    initial_type = [('float_input', FloatTensorType([None, len(feature_names)]))]
    
    onnx_model = convert_sklearn(
        model,
        initial_types=initial_type,
        target_opset=17,
        zipmap=False  # Don't output class probabilities
    )
    
    return onnx_model

def convert_to_json_format(model, feature_names):
    """
    Convert decision tree to zkml-soroban JSON format.
    
    This creates a simplified JSON representation that matches the
    schema expected by zkml-prover::model_io::import_json.
    """
    tree = model.tree_
    
    nodes = []
    n_nodes = tree.node_count
    
    for i in range(n_nodes):
        if tree.feature[i] != -2:  # Internal node (split)
            nodes.append({
                "type": "split",
                "feature_index": int(tree.feature[i]),
                "threshold": float(tree.threshold[i]),
                "left": int(tree.children_left[i]),
                "right": int(tree.children_right[i])
            })
        else:  # Leaf node
            # Get the majority class for this leaf
            class_counts = tree.value[i][0]
            majority_class = int(np.argmax(class_counts))
            nodes.append({
                "type": "leaf",
                "value": float(majority_class)
            })
    
    json_model = {
        "kind": "decision_tree",
        "num_features": len(feature_names),
        "nodes": nodes
    }
    
    return json_model

def main():
    print("Generating synthetic KYC dataset...")
    df = generate_synthetic_kyc_data(n_samples=1000)
    
    # Save dataset
    dataset_path = "kyc_dataset.csv"
    df.to_csv(dataset_path, index=False)
    print(f"Saved dataset to {dataset_path}")
    print(f"Dataset shape: {df.shape}")
    print(f"Risk tier distribution:\n{df['risk_tier'].value_counts()}\n")
    
    print("Training decision tree model...")
    model = train_model(df)
    
    feature_names = df.drop('risk_tier', axis=1).columns.tolist()
    
    # Export to ONNX
    print("Exporting to ONNX format...")
    onnx_model = export_to_onnx(model, feature_names)
    onnx_path = "kyc_decision_tree.onnx"
    with open(onnx_path, "wb") as f:
        f.write(onnx_model.SerializeToString())
    print(f"Saved ONNX model to {onnx_path}")
    
    # Convert to JSON format for zkml-soroban
    print("Converting to zkml-soroban JSON format...")
    json_model = convert_to_json_format(model, feature_names)
    json_path = "kyc_decision_tree.json"
    with open(json_path, "w") as f:
        json.dump(json_model, f, indent=2)
    print(f"Saved JSON model to {json_path}")
    
    print("\n✓ Training and export complete!")
    print(f"  - Dataset: {dataset_path}")
    print(f"  - ONNX model: {onnx_path}")
    print(f"  - JSON model: {json_path}")
    print(f"  - Features: {len(feature_names)}")
    print(f"  - Tree nodes: {len(json_model['nodes'])}")

if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
