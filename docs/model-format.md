# JSON Model Exchange Format

The CLI and example models use a JSON exchange format via
`model_io::import_json`. The ONNX protobuf foundation (`import_onnx`) already
validates opset and operators; parameter extraction is tracked in issues #5 /
#6. See [onnx-import.md](onnx-import.md).

The JSON schema mirrors the in-memory model types.

## Logistic Regression

```json
{ "kind": "logistic_regression", "weights": [0.4, -1.2, 0.8], "bias": 0.1 }
```

## Decision Tree

```json
{
  "kind": "decision_tree",
  "num_features": 2,
  "nodes": [
    { "type": "split", "feature_index": 0, "threshold": 0.5, "left": 1, "right": 2 },
    { "type": "leaf", "value": 0.0 },
    { "type": "leaf", "value": 1.0 }
  ]
}
```

## Tiny MLP

```json
{
  "kind": "tiny_mlp",
  "layers": [
    { "weights": [1.0], "biases": [0.0], "input_size": 1, "output_size": 1 }
  ]
}
```

All floating-point values are quantized to `Q16.16` on import.
