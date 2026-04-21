# Development Roadmap

This document outlines the phased development plan for zkml-soroban, from
an initial proof-of-concept to a production-ready SDK.

---

## Timeline Overview

| Phase   | Name              | Duration   | Target Completion     |
|---------|-------------------|------------|-----------------------|
| Phase 1 | MVP               | ~6 weeks   | June 2026             |
| Phase 2 | Differentiation   | ~3 months  | September 2026        |
| Phase 3 | Ecosystem         | Ongoing    | Q4 2026+              |

---

## Phase 1: Minimum Viable Product

**Objective:** Demonstrate end-to-end provable ML inference on Stellar testnet
using a decision tree model for KYC risk scoring.

**Implementation route:** Route A (RISC Zero zkVM)

### Milestones

| ID   | Milestone                        | Deliverable                              | Status      |
|------|----------------------------------|------------------------------------------|-------------|
| 1.1  | Project scaffold                 | Cargo workspace, crate structure, CI     | In Progress |
| 1.2  | Fixed-point arithmetic library   | `zkml-common::fixed_point` with tests    | In Progress |
| 1.3  | Model representations            | Decision tree and logistic regression types | In Progress |
| 1.4  | ONNX import pipeline             | Parse ONNX, extract tree/LR models       | Not Started |
| 1.5  | Inference engine                 | Execute decision tree and LR inference   | In Progress |
| 1.6  | RISC Zero integration            | Run inference inside zkVM guest          | Not Started |
| 1.7  | Groth16 proof wrapping           | Convert STARK receipt to Groth16 SNARK   | Not Started |
| 1.8  | Verifier contract                | Soroban contract with BN254 verification | In Progress |
| 1.9  | Testnet deployment               | Deploy verifier, run end-to-end demo     | Not Started |
| 1.10 | Documentation                    | Architecture, API docs, usage guide      | In Progress |

### Success Criteria

- A decision tree model trained on a sample KYC dataset produces an inference
  result off-chain.
- A Groth16 proof of that inference is verified successfully by the Soroban
  contract on Stellar testnet.
- End-to-end latency (proof generation + submission + verification) is under
  60 seconds.
- Proof size is under 500 bytes.

---

## Phase 2: Technical Differentiation

**Objective:** Replace the generic zkVM prover with native ZK circuits
optimized for the three target model architectures, achieving significantly
smaller proofs and cheaper verification.

**Implementation route:** Route B (native BN254 + Poseidon circuits)

### Milestones

| ID   | Milestone                        | Deliverable                              | Status      |
|------|----------------------------------|------------------------------------------|-------------|
| 2.1  | Decision tree circuit            | Native BN254 circuit for tree traversal  | Not Started |
| 2.2  | Logistic regression circuit      | Linear algebra circuit with fixed-point  | Not Started |
| 2.3  | Tiny MLP circuit                 | Dense layer + quantized ReLU circuit     | Not Started |
| 2.4  | Poseidon model commitments       | Use CAP-0075 host functions directly     | Not Started |
| 2.5  | Benchmark suite                  | Compare Route A vs Route B (proof size, gas, latency) | Not Started |
| 2.6  | Verifier contract v2             | Optimized for native circuit proofs      | Not Started |
| 2.7  | Recursive proof exploration      | Evaluate Nova/SuperNova for batch proofs | Not Started |

### Success Criteria

- Native circuit proofs are at least 10x smaller than Route A proofs for
  decision trees.
- On-chain verification gas cost is reduced by at least 50% compared to
  Phase 1.
- All three model types have functional circuits with test vectors.

### Technical Notes

- Decision trees have highly regular structure (binary branching), making them
  ideal for circuit optimization. Each node becomes a conditional constraint.
- Logistic regression is linear and translates directly to a small number of
  multiplication and addition gates.
- MLP circuits require implementing quantized ReLU as `max(0, x)`, which can
  be expressed using a comparison gadget and a conditional selection.
- Recursive proof schemes (Nova/SuperNova/HyperNova) will be evaluated for
  cases where multiple inference results need to be batched into a single
  on-chain verification.

---

## Phase 3: Ecosystem and Adoption

**Objective:** Package zkml-soroban as a reusable SDK and pursue ecosystem
integration through developer tooling, documentation, and grant funding.

### Milestones

| ID   | Milestone                        | Deliverable                              | Status      |
|------|----------------------------------|------------------------------------------|-------------|
| 3.1  | SDK packaging                    | Published crate with stable API          | Not Started |
| 3.2  | Developer documentation          | Integration guides, API reference, tutorials | Not Started |
| 3.3  | Example applications             | Reference implementations for each use case | Not Started |
| 3.4  | SDF grant proposal               | Formal proposal aligned with privacy roadmap | Not Started |
| 3.5  | Protocol 26 integration          | Leverage Yardstick benchmarking tools    | Not Started |
| 3.6  | Community engagement             | Conference talks, blog posts, developer outreach | Not Started |

### Success Criteria

- At least one external project integrates zkml-soroban for provable
  inference.
- SDK is published to crates.io with stable versioning.
- SDF grant proposal is submitted and accepted.

---

## Dependencies and Risks

| Risk                                      | Mitigation                                    |
|-------------------------------------------|-----------------------------------------------|
| RISC Zero API changes                     | Pin specific versions, maintain adapter layer |
| BN254 host function limitations           | Test early on testnet, engage SDF if needed   |
| Proof generation latency too high         | Use Bonsai proving service for acceleration   |
| Model accuracy loss from quantization     | Validate quantized model against original     |
| Soroban contract size limits              | Keep verifier minimal, offload logic off-chain|
| Circuit audit requirements for Phase 2    | Plan for third-party audit before mainnet     |

---

## Related Resources

- [Stellar CAP-0074: BN254 Host Functions](https://stellar.org/protocol/cap-0074)
- [Stellar CAP-0075: Poseidon Hash Functions](https://stellar.org/protocol/cap-0075)
- [RISC Zero Documentation](https://dev.risczero.com/)
- [stellar-zk by Nethermind](https://github.com/nicholasgasior/stellar-zk)
- [Boundless x Google Cloud ZK AI Partnership](https://risczero.com/boundless)
