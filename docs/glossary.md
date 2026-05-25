# Glossary

| Term | Meaning |
| ---- | ------- |
| **Groth16** | A succinct zero-knowledge proof system with constant-size proofs and fast verification via elliptic curve pairings. |
| **BN254** | A pairing-friendly elliptic curve used by Groth16; exposed as Soroban host functions under CAP-0074. |
| **Poseidon** | A ZK-friendly hash function, cheap to evaluate inside a circuit; exposed under CAP-0075. |
| **Fixed-point (Q16.16)** | Integer encoding of real numbers with 16 fractional bits, used because circuits cannot represent floats. |
| **Quantization** | The process of converting floating-point model parameters into fixed-point. |
| **Commitment** | A short binding value (hash) that ties a proof to a specific model and input. |
| **zkVM** | A virtual machine that produces a proof of correct execution of a program (RISC Zero in Phase 1). |
| **Public inputs** | Values revealed to the verifier alongside a proof. |
