#!/usr/bin/env python3
"""
Generate synthetic KYC dataset for demo purposes.

This script creates a synthetic dataset with 5-15 features representing
typical KYC assessment factors. The data is synthetic and contains no real PII.
"""

import numpy as np
import pandas as pd
from pathlib import Path

# Set random seed for reproducibility
np.random.seed(42)

# Number of samples
N_SAMPLES = 1000

# Feature definitions (5-15 features as specified in use-cases.md)
FEATURES = [
    "age",                    # Age in years (18-80)
    "account_age_days",       # Account age in days (0-3650)
    "transaction_count_30d",  # Transaction count last 30 days (0-100)
    "avg_transaction_amount", # Average transaction amount (0-10000)
    "has_verified_doc",       # Document verification status (0 or 1)
    "jurisdiction_risk_score",# Jurisdiction risk score (0-100)
    "login_frequency_30d",    # Login frequency last 30 days (0-50)
    "device_trust_score",     # Device trust score (0-100)
    "email_domain_age_days",  # Email domain age in days (0-3650)
    "phone_verified",         # Phone verification status (0 or 1)
]

def generate_synthetic_kyc_data():
    """Generate synthetic KYC dataset."""
    data = {}
    
    # Age: normal distribution around 40, range 18-80
    data["age"] = np.clip(np.random.normal(40, 15, N_SAMPLES), 18, 80).astype(int)
    
    # Account age: exponential distribution
    data["account_age_days"] = np.random.exponential(500, N_SAMPLES).astype(int)
    
    # Transaction count: Poisson-like distribution
    data["transaction_count_30d"] = np.random.poisson(10, N_SAMPLES)
    
    # Average transaction amount: log-normal distribution
    data["avg_transaction_amount"] = np.random.lognormal(6, 1, N_SAMPLES)
    data["avg_transaction_amount"] = np.clip(data["avg_transaction_amount"], 0, 10000)
    
    # Document verification: 80% verified
    data["has_verified_doc"] = np.random.binomial(1, 0.8, N_SAMPLES)
    
    # Jurisdiction risk: uniform distribution
    data["jurisdiction_risk_score"] = np.random.uniform(0, 100, N_SAMPLES)
    
    # Login frequency: Poisson distribution
    data["login_frequency_30d"] = np.random.poisson(5, N_SAMPLES)
    
    # Device trust: normal distribution
    data["device_trust_score"] = np.clip(np.random.normal(70, 20, N_SAMPLES), 0, 100)
    
    # Email domain age: exponential distribution
    data["email_domain_age_days"] = np.random.exponential(1000, N_SAMPLES).astype(int)
    
    # Phone verification: 70% verified
    data["phone_verified"] = np.random.binomial(1, 0.7, N_SAMPLES)
    
    # Generate risk tier (0=low, 1=medium, 2=high) based on features
    # This is a simplified rule-based approach for synthetic data
    risk_scores = (
        (data["jurisdiction_risk_score"] * 0.3) +
        (100 - data["device_trust_score"]) * 0.2 +
        (100 - data["account_age_days"] / 3650 * 100) * 0.15 +
        (100 - data["email_domain_age_days"] / 3650 * 100) * 0.1 +
        (1 - data["has_verified_doc"]) * 20 +
        (1 - data["phone_verified"]) * 15
    )
    
    # Convert to risk tiers
    data["risk_tier"] = pd.cut(
        risk_scores,
        bins=[0, 30, 60, 100],
        labels=[0, 1, 2]
    ).astype(int)
    
    df = pd.DataFrame(data)
    return df

def main():
    """Generate and save synthetic KYC dataset."""
    output_dir = Path(__file__).parent
    output_file = output_dir / "kyc_dataset.csv"
    
    df = generate_synthetic_kyc_data()
    
    # Save to CSV
    df.to_csv(output_file, index=False)
    print(f"Generated synthetic KYC dataset with {len(df)} samples")
    print(f"Saved to: {output_file}")
    print(f"\nRisk tier distribution:")
    print(df["risk_tier"].value_counts().sort_index())
    print(f"\nFeature statistics:")
    print(df.describe())

if __name__ == "__main__":
    main()
