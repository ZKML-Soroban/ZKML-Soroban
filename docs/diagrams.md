# Technical Diagrams

This document contains visual representations of the zkml-soroban system
architecture, data flows, and component relationships. All diagrams use
Mermaid syntax for version-controlled rendering.

---

## Table of Contents

- [System Architecture](#system-architecture)
- [Proof Generation and Verification Sequence](#proof-generation-and-verification-sequence)
- [Model Import and Quantization Pipeline](#model-import-and-quantization-pipeline)
- [On-Chain Verification Flow](#on-chain-verification-flow)
- [Route A vs Route B Comparison](#route-a-vs-route-b-comparison)
- [Component Dependency Graph](#component-dependency-graph)
- [Decision Tree Circuit Structure](#decision-tree-circuit-structure)

---

## System Architecture

```mermaid
graph TB
    subgraph "Off-Chain Environment"
        ONNX["ONNX Model File"]
        IMP["ONNX Importer"]
        QUANT["Quantization Engine"]
        INF["Inference Engine"]
        PROVE["Proof Generator"]
    end

    subgraph "Stellar Network"
        CONTRACT["zkml-verifier Contract"]
        LEDGER["Stellar Ledger"]
        BN254["BN254 Host Functions"]
        POSEIDON["Poseidon Host Functions"]
    end

    ONNX --> IMP
    IMP --> QUANT
    QUANT --> INF
    INF --> PROVE
    PROVE -->|"Groth16 Proof + Public Inputs"| CONTRACT
    CONTRACT --> BN254
    CONTRACT --> POSEIDON
    CONTRACT -->|"InferenceRecord"| LEDGER
```

---

## Proof Generation and Verification Sequence

```mermaid
sequenceDiagram
    participant User
    participant Prover as zkml-prover
    participant zkVM as RISC Zero zkVM
    participant Contract as zkml-verifier
    participant Stellar as Stellar Network

    User->>Prover: Submit model + input features
    Prover->>Prover: Load quantized model
    Prover->>Prover: Compute Poseidon hash of model
    Prover->>Prover: Compute Poseidon hash of inputs
    Prover->>zkVM: Execute inference as guest program
    zkVM->>zkVM: Generate STARK proof
    zkVM->>Prover: Return receipt (STARK)
    Prover->>Prover: Wrap STARK into Groth16 SNARK
    Prover->>Contract: invoke verify_inference(proof, public_inputs)
    Contract->>Stellar: Call BN254 pairing check host function
    Stellar-->>Contract: Pairing result (valid/invalid)
    alt Proof is valid
        Contract->>Contract: Store InferenceRecord
        Contract-->>User: Return true
    else Proof is invalid
        Contract-->>User: Return false
    end
```

---

## Model Import and Quantization Pipeline

```mermaid
flowchart LR
    A["Trained Model\n(PyTorch / scikit-learn)"] --> B["Export to ONNX"]
    B --> C["ONNX Parser\n(prost protobuf)"]
    C --> D{"Detect Model Type"}
    D -->|TreeEnsembleClassifier| E["Extract tree nodes\n& thresholds"]
    D -->|LinearClassifier| F["Extract weights\n& bias"]
    D -->|MatMul + Add + ReLU| G["Extract layer\nparameters"]
    E --> H["Quantize to\nFixedPoint Q16.16"]
    F --> H
    G --> H
    H --> I["Internal Model\nRepresentation"]
    I --> J["Compute Poseidon\nmodel commitment"]
```

---

## On-Chain Verification Flow

```mermaid
flowchart TD
    A["Receive verify_inference call"] --> B{"Contract\ninitialized?"}
    B -->|No| C["Panic: not initialized"]
    B -->|Yes| D["Deserialize proof points\nA, B, C"]
    D --> E["Reconstruct public\ninput vector"]
    E --> F["Call BN254 pairing\nhost function"]
    F --> G{"Pairing equation\nsatisfied?"}
    G -->|No| H["Return false"]
    G -->|Yes| I["Create InferenceRecord"]
    I --> J["Store record in\ninstance storage"]
    J --> K["Return true"]
```

---

## Route A vs Route B Comparison

```mermaid
graph LR
    subgraph "Route A: RISC Zero zkVM"
        A1["Rust inference code"] --> A2["zkVM Guest Execution"]
        A2 --> A3["STARK Proof"]
        A3 --> A4["Groth16 Wrapping"]
        A4 --> A5["~300-500 byte proof"]
    end

    subgraph "Route B: Native Circuits"
        B1["Model-specific circuit\n(bellman / halo2)"] --> B2["Direct constraint\nsatisfaction"]
        B2 --> B3["Native Groth16 Proof"]
        B3 --> B4["~100-200 byte proof"]
    end

    A5 --> V["Soroban Verifier\n(BN254 pairing check)"]
    B4 --> V
```

---

## Component Dependency Graph

```mermaid
graph TD
    COMMON["zkml-common\n(shared types)"]
    PROVER["zkml-prover\n(off-chain)"]
    VERIFIER["zkml-verifier\n(on-chain)"]
    SERDE["serde"]
    RISC0["risc0-zkvm"]
    SOROBAN["soroban-sdk"]
    BN254_HOST["BN254 Host Functions\n(CAP-0074)"]
    POSEIDON_HOST["Poseidon Host Functions\n(CAP-0075)"]

    PROVER --> COMMON
    PROVER --> SERDE
    PROVER --> RISC0
    VERIFIER --> SOROBAN
    VERIFIER --> BN254_HOST
    VERIFIER --> POSEIDON_HOST
    COMMON --> SERDE
```

---

## Decision Tree Circuit Structure

This diagram illustrates how a simple decision tree is encoded as
arithmetic constraints in a ZK circuit (Route B).

```mermaid
graph TD
    ROOT["Node 0: Split\nfeature[2] <= 0.5?"]
    ROOT -->|"Yes (left)"| N1["Node 1: Split\nfeature[0] <= 0.3?"]
    ROOT -->|"No (right)"| N2["Node 2: Leaf\nclass = 1"]
    N1 -->|"Yes (left)"| N3["Node 3: Leaf\nclass = 0"]
    N1 -->|"No (right)"| N4["Node 4: Leaf\nclass = 1"]

    style ROOT fill:#334155,stroke:#94a3b8,color:#e2e8f0
    style N1 fill:#334155,stroke:#94a3b8,color:#e2e8f0
    style N2 fill:#065f46,stroke:#34d399,color:#d1fae5
    style N3 fill:#065f46,stroke:#34d399,color:#d1fae5
    style N4 fill:#065f46,stroke:#34d399,color:#d1fae5
```

Each split node becomes a comparison constraint in the circuit:
- The prover provides a binary selector `s_i` for each node, indicating the
  traversal direction.
- The circuit enforces `s_i = 1` if `feature[j] <= threshold` and `s_i = 0`
  otherwise.
- A path selector aggregates the binary decisions to identify the reached
  leaf.
- The circuit constrains the output to equal the value of the selected leaf.
