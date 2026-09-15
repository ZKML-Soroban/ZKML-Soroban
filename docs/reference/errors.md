---
title: "Errors"
description: "Error types of the verifier contract, the shared library, the ONNX importer, and the CLI."
icon: "triangle-exclamation"
---

## VerificationError (contract)

Returned by `verify_inference` as a Soroban contract error.

| Code | Variant                         | Cause                                                        |
| ---- | ------------------------------- | ------------------------------------------------------------ |
| 1    | `ContractNotInitialized`        | `initialize` has not been called                             |
| 2    | `PublicInputsTooShort`          | Public inputs end before a required field                    |
| 3    | `MalformedProofA`               | `proof_a` is not 64 bytes (also returned for a wrong-length `proof_c`) |
| 4    | `MalformedProofB`               | `proof_b` is not 128 bytes                                   |
| 5    | `MalformedProofC`               | Reserved; wrong-length `proof_c` currently reports code 3    |
| 6    | `MalformedVerificationKey`      | A verification key point has the wrong length              |
| 7    | `VerificationFailed`            | Pairing check failed, `model_hash` mismatch, or contract paused |
| 8    | `InvalidPublicInputLength`      | Extra bytes after the 80-byte layout                         |
| 9    | `VerificationKeyLengthMismatch` | `vk.ic` does not have exactly 5 points                       |
| 10   | `ProofAlreadyUsed`              | The nullifier for these public inputs already exists         |

Panics (not error codes):

| Function                          | Message                                   |
| --------------------------------- | ----------------------------------------- |
| `initialize` (second call)        | `contract is already initialized`         |
| `get_result` (nothing recorded)   | `no inference result has been recorded yet` |
| `get_model_hash`, `get_admin`, admin setters before init | `contract is not initialized` |
| Admin setters without admin auth  | Soroban auth failure                      |

## ZkmlError (zkml-common)

| Variant                                   | Cause                                         |
| ----------------------------------------- | --------------------------------------------- |
| `FeatureCountMismatch { expected, got }`  | Input length differs from the model           |
| `InvalidModel(String)`                    | Structural validation failed                  |
| `ParseError(String)`                      | External model representation failed to parse |
| `ArithmeticOverflow`                      | A fixed-point operation overflowed            |
| `QuantizationError(String)`               | Quantization validation failed                |

## OnnxImportError (zkml-prover)

| Variant                                  | Cause                                                          |
| ---------------------------------------- | -------------------------------------------------------------- |
| `MalformedModel(String)`                 | Decode failure, missing graph, empty nodes, missing opsets, extraction failure |
| `UnsupportedOpset { found, required }`   | A known domain is below its opset floor                        |
| `UnsupportedOperator { op_type }`        | Operator outside the allowlist                                 |
| `ExtractionNotImplemented { .. }`        | Validated graph without an extractor                           |

## CLI exit codes

| Code | Meaning                                                      |
| ---- | ------------------------------------------------------------ |
| 0    | Success                                                      |
| 1    | IO, import, validation, or inference failure                 |
| 2    | Invalid arguments or malformed `--input`                     |
