# ONNX Import

## Status

The importer foundation is implemented in `crates/zkml-prover/src/onnx/`:

1. Decode ONNX protobuf (`ModelProto`) via `prost`.
2. Require opset **>= 17** for the `ai.onnx` / `ai.onnx.ml` domains.
3. Walk graph nodes and allow only the supported operator set.
4. Return a typed [`OnnxImportError`](../crates/zkml-prover/src/onnx/error.rs).

**Parameter extraction is not implemented yet.** After validation succeeds,
`import_onnx` returns `ExtractionNotImplemented` until:

- [Issue #5](https://github.com/ZKML-Soroban/ZKML-Soroban/issues/5) — `TreeEnsembleClassifier` → `DecisionTree`
- [Issue #6](https://github.com/ZKML-Soroban/ZKML-Soroban/issues/6) — `LinearClassifier` → `LogisticRegression`

For end-to-end demos today, use the JSON exchange format via
`model_io::import_json` (see [model-format.md](model-format.md)).

## Public API

```rust
pub fn import_onnx(bytes: &[u8]) -> Result<Model, OnnxImportError>;
```

### Error variants

| Variant | When |
| ------- | ---- |
| `MalformedModel` | Protobuf decode failure, missing graph, empty nodes, missing opset imports |
| `UnsupportedOpset { found, required }` | Relevant opset below 17 |
| `UnsupportedOperator { op_type }` | Node uses an op outside the allowlist |
| `ExtractionNotImplemented { architecture_hint }` | Validation OK; extraction pending (#5 / #6) |

## Operator mapping

| ONNX Operator            | Internal Model        | Extraction |
| ------------------------ | --------------------- | ---------- |
| `TreeEnsembleClassifier` | `DecisionTree`        | Issue #5   |
| `LinearClassifier`       | `LogisticRegression`  | Issue #6   |
| `MatMul` + `Add` + `Relu`| `TinyMLP`             | Future     |

Unsupported operators fail at import time with a message that names the
offending `op_type` (never silent ignore).

## Pipeline

1. Deserialize the ONNX protobuf into a graph.
2. Validate opset imports (`>= 17`).
3. Validate every node against the operator allowlist.
4. *(Pending)* Identify the architecture and extract weights / tree structure.
5. *(Pending)* Quantize floating-point values to `Q16.16`.
6. *(Pending)* Return the matching `Model` variant.

## Fixtures

See `crates/zkml-prover/tests/fixtures/README.md` for committed test models
and regeneration instructions.
