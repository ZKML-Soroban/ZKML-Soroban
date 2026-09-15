---
title: "JSON model format"
description: "The JSON exchange format accepted by the CLI and model_io::import_json."
icon: "brackets-curly"
---

The CLI and the example models use a JSON exchange format loaded with
`zkml_prover::model_io::import_json`. The schema mirrors the in-memory model
types. All floating-point values are quantized to Q16.16 on import.

For ONNX files, see [ONNX import](/guides/onnx-import).

## Logistic regression

```json
{
  "kind": "logistic_regression",
  "weights": [0.82, -0.41, 0.33, 1.07],
  "bias": -0.2,
  "decision_threshold": 0.0
}
```

`class_label` is `1` when the raw score is `>= decision_threshold`.

## Decision tree

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

Node `0` is the root. A split goes `left` when `feature <= threshold`.

## Tiny MLP

```json
{
  "kind": "tiny_mlp",
  "layers": [
    { "weights": [1.0, 0.5, -0.5, 1.0], "biases": [0.0, 0.1], "input_size": 2, "output_size": 2 },
    { "weights": [1.0, -1.0], "biases": [0.0], "input_size": 2, "output_size": 1 }
  ]
}
```

Weights are row-major (`weights[j * input_size + i]`). ReLU is applied after
every layer except the last.
