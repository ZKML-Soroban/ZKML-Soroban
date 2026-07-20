# ONNX Import

## Status

The importer foundation is implemented in `crates/zkml-prover/src/onnx/`:

1. Decode ONNX protobuf (`ModelProto`) via `prost`.
2. Validate **per-domain** opset floors (see below).
3. Walk graph nodes and allow only the supported operator set.
4. Return a typed [`OnnxImportError`](../crates/zkml-prover/src/onnx/error.rs).

**Parameter extraction is not implemented yet.** After validation succeeds,
`import_onnx` returns `ExtractionNotImplemented` until:

- [Issue #5](https://github.com/ZKML-Soroban/ZKML-Soroban/issues/5) — `TreeEnsembleClassifier` → `DecisionTree`
- [Issue #6](https://github.com/ZKML-Soroban/ZKML-Soroban/issues/6) — `LinearClassifier` → `LogisticRegression`

For end-to-end demos today, use the JSON exchange format via
`model_io::import_json` (see [model-format.md](model-format.md)).

## Opset floors (per domain)

ONNX domains version independently. The importer does **not** apply a single
global floor to every domain:

| Domain | Minimum | Notes |
| ------ | ------- | ----- |
| `""` / `ai.onnx` | **17** | Core operators (`MatMul`, `Add`, `Relu`, …) |
| `ai.onnx.ml` | **1** | Classic ML ops (`TreeEnsembleClassifier` 1/3/5, `LinearClassifier` 1 only). There is no ml opset 17. |

Real skl2onnx exports typically declare pairs such as `("", 17)` +
`("ai.onnx.ml", 1..5)`. Applying core's 17 floor to the ML domain would reject
every tree and linear model this project targets.

Other domains are ignored for floor checks but the model must still declare at
least one of the known domains above.

## Public API

```rust
pub fn import_onnx(bytes: &[u8]) -> Result<Model, OnnxImportError>;
```

### Error variants

| Variant | When |
| ------- | ---- |
| `MalformedModel` | Protobuf decode failure, missing graph, empty nodes, missing opset imports |
| `UnsupportedOpset { found, required }` | A known domain is below its floor (`required` is that domain's floor) |
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
2. Validate opset imports per domain (core >= 17, ml >= 1).
3. Validate every node against the operator allowlist.
4. *(Pending)* Identify the architecture and extract weights / tree structure.
5. *(Pending)* Quantize floating-point values to `Q16.16`.
6. *(Pending)* Return the matching `Model` variant.

## Fixtures

See `crates/zkml-prover/tests/fixtures/README.md` for committed test models
and regeneration instructions.
