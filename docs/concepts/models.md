---
title: "Models"
description: "Decision trees, logistic regression, and tiny MLPs in fixed-point form."
icon: "diagram-project"
---

All models live in `zkml_common::models`, use `FixedPoint` parameters, and are
wrapped in the `Model` enum.

## Decision tree

A flat vector of nodes, root at index 0.

- `Split { feature_index, threshold, left, right }` goes left when
  `feature <= threshold`.
- `Leaf { value }` carries the prediction.

`DecisionTree::validate` rejects out-of-range children, self references, cycles,
and unreachable nodes. Traversal is bounded by an iteration limit so a malformed
tree cannot loop forever.

**Decision:** `class_label` is always `0`; the prediction is the leaf value in
`output`.

## Logistic regression

A weight vector, a bias, and a `decision_threshold`. Inference computes the raw
linear score `w . x + b` with checked `i128` accumulation; the sigmoid is
omitted because comparing the raw score to a threshold gives the same binary
decision.

**Decision:** `class_label = 1` if `score >= decision_threshold`, else `0`.

## Tiny MLP

An ordered list of `DenseLayer { weights, biases, input_size, output_size }`.
Weights are row-major: `weights[j * input_size + i]`. ReLU is applied after every
hidden layer; the final layer emits raw logits. `TinyMLP::validate` checks
layer shapes and chaining.

**Decision:** `class_label = argmax(logits)` (lowest index on ties); `output`
is the first logit.

## Inference API

| Function                        | Behavior                                        |
| ------------------------------- | ----------------------------------------------- |
| `run_inference`                 | Returns the score; panics on invalid input.     |
| `try_run_inference`             | Returns `Result<FixedPoint, ZkmlError>`.         |
| `run_inference_with_decision`   | Returns `(score, class_label)`; panics on invalid input, overflow, or the tree iteration limit. |
| `run_batch` / `try_run_batch`   | Batch variants; `try_run_batch` returns one `Result` per row. |

The same functions run natively and inside the zkVM guest.
