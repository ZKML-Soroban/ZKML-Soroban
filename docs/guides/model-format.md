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

## From scikit-learn

`tools/sklearn_to_json.py` converts a pickled estimator into this format:

```bash
python tools/sklearn_to_json.py model.pkl -o model.json
```

It handles `LogisticRegression` with a single output and
`DecisionTreeClassifier`. Anything else stops with a message naming what it
found, rather than writing a file that looks right and is not.

Check the result with the CLI:

```bash
cargo run -p zkml-prover -- inspect model.json
cargo run -p zkml-prover -- infer model.json --input="0.5,0.2,0.1,0.9"
```

Use `--input=` with an equals sign when a feature is negative, otherwise the
leading minus is read as a flag.

Two things carry over unchanged, so the conversion is a relabelling rather than
a translation: scikit-learn sends a sample left when `feature <= threshold`,
which is the rule here, and its tree arrays already put the root at index `0`
with children referenced by index. A leaf keeps the class with the largest
count at that node, mapped back through `classes_`.

Expect the scores to differ from scikit-learn in the fifth decimal or so. The
weights and the inputs are quantized to Q16.16, whose step is `1/65536`.

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
