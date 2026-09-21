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
| 11   | `MalformedSeal`                 | The seal is not 260 bytes                                    |
| 12   | `UnknownSelector`               | The seal was made for a different RISC Zero version          |
| 13   | `MalformedJournal`              | The journal is not the 96-byte `JournalV1` layout, or its magic or version do not match |
| 14   | `Risc0NotConfigured`            | `set_risc0_config` or `set_risc0_vk` has not been called     |

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

## ProveError (zkml-prover)

Returned by the proving pipeline. Every variant names what to do about it.

| Variant                              | Cause and fix                                                     |
| ------------------------------------ | ----------------------------------------------------------------- |
| `Inference(ZkmlError)`               | Inference failed before proving started                            |
| `Journal(JournalError)`              | The guest journal did not decode; prover and guest are out of sync |
| `JournalMismatch(String)`            | The guest and native inference disagree. A determinism bug: do not ship the bundle |
| `Bundle(BundleError)`                | The bundle could not be assembled (bad seal length, bad journal)   |
| `Zkvm(String)`                       | The zkVM or the receipt verification failed                        |
| `UnsupportedPlatform { os, arch }`   | Groth16 compression needs x86_64 Linux                             |
| `DockerUnavailable(String)`          | Docker is not installed or not running                             |
| `DevModeHasNoSeal`                   | `RISC0_DEV_MODE=1` produces fake receipts. Unset it for a real proof |
| `RemoteBackend(String)`              | Remote proving is not implemented; use `--backend local`           |
| `Serialization(String)`              | The bundle could not be written or read as JSON                    |

## JournalError and BundleError (zkml-common)

| Variant                              | Cause                                                    |
| ------------------------------------ | -------------------------------------------------------- |
| `JournalError::BadMagic`             | The journal does not start with `ZKML`                    |
| `JournalError::UnsupportedVersion`   | Journal version this build does not know                  |
| `JournalError::InvalidLength`            | The journal is not 96 bytes                               |
| `JournalError::UnknownModelKind`     | Model kind byte outside the known range                   |
| `BundleError::BadSealLength`         | The seal is not 260 bytes                                 |
| `BundleError::UnsupportedVersion`    | Bundle version this build does not know                   |
| `BundleError::UnknownProofSystem`    | Proof system id outside the known range                   |
| `BundleError::Truncated`             | The binary encoding ended early                           |
| `BundleError::BadMagic`              | The binary encoding does not start with `ZKMLBNDL`        |

## CLI exit codes

| Code | Meaning                                                      |
| ---- | ------------------------------------------------------------ |
| 0    | Success                                                      |
| 1    | IO, import, validation, or inference failure                 |
| 2    | Invalid arguments or malformed `--input`                     |
