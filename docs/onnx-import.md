# ONNX Import

## Status

Native ONNX protobuf import is planned. Today the import entrypoint
(`import_onnx`) accepts the JSON exchange format described in
[model-format.md](model-format.md), which keeps the rest of the pipeline
stable while the protobuf path is built out.

## Planned operator mapping

| ONNX Operator            | Internal Model        |
| ------------------------ | --------------------- |
| `TreeEnsembleClassifier` | `DecisionTree`        |
| `LinearClassifier`       | `LogisticRegression`  |
| `MatMul` + `Add` + `Relu`| `TinyMLP`             |

## Pipeline

1. Deserialize the ONNX protobuf into a graph.
2. Identify the architecture from the operator set.
3. Extract weights, biases, and tree structures.
4. Quantize every floating-point value to `Q16.16`.
5. Return the matching `Model` variant.
