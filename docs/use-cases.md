# Use Cases

This document describes the primary use cases targeted by zkml-soroban,
explaining why each is a natural fit for provable ML inference on the
Stellar network.

---

## Table of Contents

- [Overview](#overview)
- [Use Case 1: Provable KYC Risk Scoring](#use-case-1-provable-kyc-risk-scoring)
- [Use Case 2: Invoice Risk Assessment for RWA Factoring](#use-case-2-invoice-risk-assessment-for-rwa-factoring)
- [Use Case 3: Privacy-Preserving Credit Scoring](#use-case-3-privacy-preserving-credit-scoring)
- [Cross-Cutting Themes](#cross-cutting-themes)

---

## Overview

Stellar is the network with the largest number of institutional anchors
focused on remittances, cross-border payments, and real-world asset (RWA)
tokenization. The Stellar Development Foundation has explicitly prioritized
compliance-ready features, including viewing keys for authorized parties and
support for institutional flows such as payroll and B2B transfers.

These characteristics make Stellar the ideal platform for ML applications
where trust, verifiability, and privacy intersect.

The three use cases below share a common pattern:

1. A model makes a decision that has financial or regulatory consequences.
2. The decision must be verifiable by counterparties who do not have access
   to the model or the input data.
3. Zero-knowledge proofs bridge this gap by proving correctness without
   revealing sensitive information.

---

## Use Case 1: Provable KYC Risk Scoring

### Problem

Financial institutions and compliant anchors on Stellar must perform
Know Your Customer (KYC) assessments before onboarding users or processing
transactions above certain thresholds. These assessments often use ML models
to classify users into risk tiers (low, medium, high).

However, the resulting risk score is currently opaque to other network
participants. A receiving anchor has no way to verify that the sending
anchor's KYC assessment was performed correctly, or even that it was
performed at all. This forces each anchor to duplicate the assessment,
increasing costs and friction.

### Solution with zkml-soroban

A KYC scoring model (decision tree or logistic regression) is trained on
historical compliance data and registered on-chain via its Poseidon hash
commitment. When a user completes KYC:

1. The anchor runs the model on the user's features (age, jurisdiction,
   transaction history, document verification scores).
2. The prover generates a ZK proof that the model produced a specific risk
   score for those features.
3. The proof is verified on-chain, and the result is recorded.

Downstream anchors can query the verified result to confirm the risk tier
without seeing the underlying features or model weights. This enables
trust-minimized compliance across the Stellar network.

### Model Architecture

- **Recommended**: Decision tree (low circuit complexity, interpretable).
- **Features**: 5-15 categorical and numerical features.
- **Output**: Risk tier (0 = low, 1 = medium, 2 = high).
- **Training data**: Historical KYC decisions (institution-specific).

### Why Stellar

Stellar's anchor ecosystem is the primary distribution channel. Compliance
is not optional -- it is a prerequisite for operating on the network. A
provable scoring system reduces duplicated effort across anchors and aligns
directly with SDF's compliance roadmap.

---

## Use Case 2: Invoice Risk Assessment for RWA Factoring

### Problem

Invoice factoring on RWA platforms requires assessing the credit risk of
each invoice. Factors (buyers of receivables) need assurance that the risk
assessment was performed rigorously, but sellers may not want to reveal the
full details of their customer relationships or internal scoring models.

Current approaches rely on third-party audits or platform-level trust,
both of which introduce delays and counterparty risk.

### Solution with zkml-soroban

A risk assessment model evaluates invoices based on features such as:
debtor payment history, invoice amount, days outstanding, industry sector,
and historical default rates.

The scoring process:

1. The originator (seller) runs the risk model on invoice features.
2. A ZK proof attests that the model produced a specific risk score.
3. The proof and score are recorded on-chain alongside the tokenized
   invoice.
4. Factors can verify the score's integrity before purchasing the
   receivable.

This creates a trustless risk layer for decentralized factoring platforms
built on Stellar's RWA infrastructure.

### Model Architecture

- **Recommended**: Logistic regression (interpretable, low constraint
  count) or decision tree.
- **Features**: 8-20 features derived from invoice metadata and debtor
  history.
- **Output**: Default probability (as a fixed-point value) or risk
  category.
- **Training data**: Historical invoice payment outcomes.

### Why Stellar

Stellar has active RWA tokenization projects and a growing ecosystem of
compliant asset issuers. Invoice factoring is a natural extension of the
existing anchor and asset framework. A provable risk score reduces the
trust assumptions that currently limit decentralized factoring adoption.

---

## Use Case 3: Privacy-Preserving Credit Scoring

### Problem

Credit scoring requires access to sensitive personal financial data:
income, debt levels, payment history, and employment status. Sharing this
data with every potential lender exposes users to privacy risks and creates
regulatory compliance burdens (GDPR, CCPA, and similar frameworks).

### Solution with zkml-soroban

A credit scoring model runs entirely on the user's side (or within a
trusted prover enclave):

1. The user provides their financial features to the prover.
2. The prover executes the model and generates a ZK proof of the result.
3. The proof is submitted on-chain, demonstrating that the user's credit
   score exceeds a required threshold without revealing the score itself
   or the underlying data.

This pattern is known as a "credential proof" -- the user proves they meet
a criterion without revealing the evidence.

### Model Architecture

- **Recommended**: Tiny MLP with 2-3 hidden layers (captures non-linear
  relationships better than linear models).
- **Features**: 10-30 financial features (income, debt-to-income ratio,
  payment history length, number of open accounts).
- **Output**: Credit score (numerical) or threshold comparison (binary).
- **Training data**: Anonymized credit bureau datasets.

### Why Stellar

Boundless (a RISC Zero initiative) has partnered with Google Cloud to
enable ZK proofs of AI model outputs, specifically targeting privacy-
preserving applications. zkml-soroban brings this capability natively to
Stellar, where institutional lenders and anchor-based lending platforms
can integrate provable credit assessments directly into their transaction
flows.

---

## Cross-Cutting Themes

### Model Governance

All three use cases require a governance framework for model updates:

- Each model version is identified by its Poseidon hash commitment.
- Updating a model requires registering a new commitment on-chain.
- Historical proofs remain valid for the model version they were generated
  against.
- Audit trails are maintained through the immutable ledger record of
  model registrations and verification results.

### Regulatory Alignment

Zero-knowledge proofs provide a unique regulatory advantage: they allow
demonstrating compliance without exposing protected data. This addresses
the tension between transparency requirements (regulators want proof of
process) and privacy requirements (data protection laws restrict sharing).

### Scalability Path

For high-volume applications (e.g., scoring every invoice on a factoring
platform), recursive proof composition (planned for Phase 2) will allow
batching multiple inference proofs into a single on-chain verification.
This reduces per-proof gas costs and improves throughput without
sacrificing individual proof soundness.
