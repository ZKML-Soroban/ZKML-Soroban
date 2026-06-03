# Model Representations

All models live in `zkml-common::models` and use `FixedPoint` parameters.

## Decision Tree

A flat vector of nodes. Index 0 is the root. Each `Split` node compares a
feature against a threshold and branches to a child index; each `Leaf` carries
the predicted value. `DecisionTree::validate` checks index bounds.

## Logistic Regression

A weight vector plus a bias. Inference computes the raw linear score; the
sigmoid is intentionally omitted (see the FAQ).

## Tiny MLP

An ordered list of dense layers with quantized ReLU between them. Weights are
row-major: `weights[j * input_size + i]`. The final layer is linear.
