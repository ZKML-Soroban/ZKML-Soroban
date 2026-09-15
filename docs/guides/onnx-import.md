---
title: "ONNX import"
description: "Import decision trees, linear classifiers, and tiny MLPs from ONNX."
icon: "file-import"
---

The importer lives in `crates/zkml-prover/src/onnx/` and turns an ONNX
`ModelProto` into a quantized `Model`.

```rust
use zkml_prover::onnx::import_onnx;

let bytes = std::fs::read("model.onnx")?;
let model = import_onnx(&bytes)?;   // Result<Model, OnnxImportError>
```

## Pipeline

1. Decode the protobuf with `prost`.
2. Validate opset imports per domain.
3. Check every node against the operator allowlist.
4. Detect the architecture:
   - graph made only of `Gemm`, `MatMul`, `Add`, `Relu` (with at least one
     `Gemm` or `MatMul`) is extracted as a `TinyMLP`
   - otherwise the graph must contain exactly one node:
     `TreeEnsembleClassifier` or `LinearClassifier`
5. Extract parameters and quantize them to Q16.16.
6. Validate the resulting structure.

## Opset floors

ONNX domains are versioned independently:

| Domain            | Minimum | Notes                                                    |
| ----------------- | ------- | -------------------------------------------------------- |
| `""` / `ai.onnx`  | **17**  | Core operators (`MatMul`, `Add`, `Relu`, `Gemm`)         |
| `ai.onnx.ml`      | **1**   | Classic ML operators. There is no ml opset 17.           |

A typical skl2onnx export declares `("", 17)` together with `("ai.onnx.ml", 1..5)`.
A model with no opset imports is rejected as malformed.

> **Export with core opset 17 or later.** A model exported with
> `target_opset=12` (as the current KYC demo script does) is rejected with
> `UnsupportedOpset`.

## Supported operators

| Operator                 | Model                | Constraints                                         |
| ------------------------ | -------------------- | --------------------------------------------------- |
| `TreeEnsembleClassifier` | `DecisionTree`       | Single tree (ensembles rejected), `BRANCH_LEQ` only, structure validated |
| `LinearClassifier`       | `LogisticRegression` | Binary only; `post_transform` `NONE` or `LOGISTIC`  |
| `Gemm`                   | `TinyMLP`            | Fused dense layer                                   |
| `MatMul` + `Add`         | `TinyMLP`            | Two-node dense layer                                |
| `Relu`                   | `TinyMLP`            | Accepted but ignored; ReLU is always applied after every hidden layer and never after the last |

Unsupported operators fail with an error naming the `op_type`.

## Errors

| Variant                                  | When                                                          |
| ---------------------------------------- | ------------------------------------------------------------- |
| `MalformedModel(String)`                 | Protobuf decode failure, missing graph, empty nodes, missing opsets, extraction or structure failure |
| `UnsupportedOpset { found, required }`   | A known domain is below its floor                              |
| `UnsupportedOperator { op_type }`        | A node uses an operator outside the allowlist                  |
| `ExtractionNotImplemented { .. }`        | Validated graph with no extractor for its shape                |

## Fixtures

Committed test models and regeneration scripts live in
`crates/zkml-prover/tests/fixtures/` (`decision_tree_valid.onnx`,
`linear_classifier_valid.onnx`, `tinymlp_valid.onnx`, `low_opset_tree.onnx`,
`unsupported_conv.onnx`, `skl2onnx_like_tree.onnx`).

Tests: `tests/onnx_import.rs`, `tests/onnx_tree_extraction.rs`,
`tests/tinymlp_inference.rs`.
