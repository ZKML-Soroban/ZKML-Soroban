# zkml-soroban

**A Provable ML Inference Runtime for Stellar**

zkml-soroban is the first runtime that enables executing small machine learning
models off-chain and cryptographically verifying the correctness of their
inference on the Stellar network through Soroban smart contracts.

The project leverages the zero-knowledge cryptographic primitives introduced in
Stellar Protocol 25 (X-Ray) -- specifically BN254 elliptic curve operations
(CAP-0074) and Poseidon hash functions (CAP-0075) -- to build a complete
pipeline from model import to on-chain proof verification.

---

## Table of Contents

- [Motivation](#motivation)
- [Architecture Overview](#architecture-overview)
- [Supported Models](#supported-models)
- [Technology Stack](#technology-stack)
- [Project Structure](#project-structure)
- [Getting Started](#getting-started)
- [Development Status](#development-status)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [License](#license)

---

## Motivation

Machine learning models increasingly drive high-stakes decisions in financial
systems: credit scoring, risk assessment, and compliance checks. However, these
decisions are typically opaque -- users and counterparties cannot verify that a
claimed ML output was actually produced by a specific model on specific inputs.

Zero-knowledge proofs solve this problem. A prover can demonstrate that an ML
inference was executed correctly without revealing the model weights or input
data. The verifier (a smart contract on Stellar) confirms correctness with a
single, cheap cryptographic check.

Stellar is uniquely positioned for this application:

- **Native ZK primitives**: Protocol 25 introduced BN254 and Poseidon host
  functions, enabling efficient Groth16 proof verification directly on-chain.
- **Institutional anchors**: Stellar has the largest network of regulated
  anchors and remittance corridors, making provable compliance scoring a
  practical use case rather than a theoretical exercise.
- **Low-cost verification**: Soroban contract execution costs are orders of
  magnitude lower than comparable EVM chains, making on-chain verification
  economically viable for high-volume applications.

---

## Architecture Overview

The system consists of two primary components:

```
                    Off-chain                          On-chain
              +-------------------+             +-------------------+
              |                   |             |                   |
  ONNX Model  |   zkml-prover    |   Groth16   |  zkml-verifier    |
  ----------->|                   |   proof     |                   |
              |  1. Import model  |------------>|  1. Verify proof  |
  Input Data  |  2. Quantize      |             |  2. Check inputs  |
  ----------->|  3. Run inference |             |  3. Record result |
              |  4. Generate proof|             |                   |
              +-------------------+             +-------------------+
                        |                                 |
                        v                                 v
                   zkml-common                    Stellar Ledger
              (shared types & structures)       (immutable record)
```

For a detailed architecture description, see
[docs/architecture.md](docs/architecture.md).

---

## Supported Models

| Model Type          | Status  | Circuit Complexity | Primary Use Case      |
| ------------------- | ------- | ------------------ | --------------------- |
| Decision Tree       | Phase 1 | Low                | KYC risk scoring      |
| Logistic Regression | Phase 1 | Low                | Binary classification |
| Tiny MLP (ReLU)     | Phase 2 | Medium             | Multi-class scoring   |

All models are imported from the ONNX format and quantized to fixed-point
arithmetic for compatibility with ZK circuit constraints.

---

## Technology Stack

### Off-chain Prover

| Component         | Technology                | Purpose                             |
| ----------------- | ------------------------- | ----------------------------------- |
| Language          | Rust                      | Performance, ZK ecosystem support   |
| Model format      | ONNX                      | Interop with PyTorch, scikit-learn  |
| Arithmetic        | Fixed-point (Q16.16)      | ZK-compatible number representation |
| Proof system      | RISC Zero zkVM (Phase 1)  | Groth16 proof generation            |
| Circuit framework | bellman / halo2 (Phase 2) | Native ML circuits                  |

### On-chain Verifier

| Component        | Technology              | Purpose                           |
| ---------------- | ----------------------- | --------------------------------- |
| Platform         | Soroban (Stellar)       | Smart contract execution          |
| Compilation      | Rust to WASM            | Contract deployment               |
| Curve operations | BN254 host functions    | Groth16 pairing checks (CAP-0074) |
| Hash commitments | Poseidon host functions | Model/input binding (CAP-0075)    |

---

## Project Structure

```
zkml-soroban/
├── Cargo.toml                  Root workspace manifest
├── README.md
├── LICENSE                     Apache 2.0
├── CONTRIBUTING.md             Contribution guidelines
├── SECURITY.md                 Security policy
├── .gitignore
│
├── crates/
│   ├── zkml-common/            Shared types and utilities
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fixed_point.rs  Fixed-point arithmetic
│   │       ├── models.rs       Model representations
│   │       └── proof.rs        Proof data structures
│   │
│   ├── zkml-prover/            Off-chain prover
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── inference.rs    Model inference engine
│   │       ├── onnx.rs         ONNX model importer
│   │       ├── prover.rs       ZK proof generation
│   │       └── quantization.rs Weight quantization
│   │
│   └── zkml-verifier/          On-chain Soroban contract
│       └── src/
│           └── lib.rs          Verification contract
│
└── docs/
    ├── architecture.md         System architecture
    ├── diagrams.md             Technical diagrams
    ├── roadmap.md              Development roadmap
    ├── technical-overview.md   ZK primitives and stack details
    └── use-cases.md            Target use cases
```

---

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.79 or later)
- [Stellar CLI](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup)
- wasm32-unknown-unknown target:
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

### Build

```bash
# Build all workspace crates
cargo build

# Build the verifier contract for deployment
cargo build --release --target wasm32-unknown-unknown -p zkml-verifier

# Run all tests
cargo test --workspace
```

### Quick Start

```bash
# Clone the repository
git clone https://github.com/diegoveme/ZKML-Soroban.git
cd ZKML-Soroban

# Build the project
cargo build

# Run tests
cargo test --workspace
```

---

## Development Status

This project is in active early development.

| Phase   | Description                      | Status      |
| ------- | -------------------------------- | ----------- |
| Phase 1 | MVP with RISC Zero prover        | In Progress |
| Phase 2 | Native BN254 + Poseidon circuits | Planned     |
| Phase 3 | SDK and ecosystem integration    | Planned     |

See [docs/roadmap.md](docs/roadmap.md) for the full development roadmap.

---

## Documentation

| Document                                         | Description                              |
| ------------------------------------------------ | ---------------------------------------- |
| [Architecture](docs/architecture.md)             | System design and component breakdown    |
| [Technical Overview](docs/technical-overview.md) | ZK primitives, stack, and design details |
| [Diagrams](docs/diagrams.md)                     | Visual system and flow diagrams          |
| [Roadmap](docs/roadmap.md)                       | Phased development plan                  |
| [Use Cases](docs/use-cases.md)                   | Target applications on Stellar           |

---

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md)
for development setup, coding standards, and the pull request process.

For security vulnerabilities, follow the process described in
[SECURITY.md](SECURITY.md).

---

## License

This project is licensed under the Apache License 2.0. See
[LICENSE](LICENSE) for the full text.

---

## Acknowledgments

- [Stellar Development Foundation](https://stellar.org/) for the ZK-native
  protocol upgrades (CAP-0074, CAP-0075).
- [Nethermind](https://nethermind.io/) for the RISC Zero zkVM deployment on
  Soroban and the `stellar-zk` reference implementation.
- [RISC Zero](https://risczero.com/) for the general-purpose zkVM.

---

## Further Documentation

A full documentation index is available in [docs/README.md](docs/README.md),
covering the model format, commitments, the proving pipeline, the verifier
interface, the threat model, and the testing guide.

## Status Update (June 2026)

The off-chain pipeline and the on-chain interface are feature-complete for the
supported model families and exercised by the test suite. The remaining Phase 1
work is the cryptographic integration (RISC Zero proving and the BN254 pairing
check), tracked in the [roadmap](docs/roadmap.md).
