#!/usr/bin/env python3
"""
Convert a trained scikit-learn model into the JSON exchange format.

    python tools/sklearn_to_json.py model.pkl -o model.json

Supported today: LogisticRegression with a single output, and
DecisionTreeClassifier. Anything else stops with a message that names what it
found, because a silent wrong conversion is worse than a refusal.

The format is documented in docs/guides/model-format.md.

Loading a pickle runs code. joblib.load and pickle.load execute whatever the
file carries, so only convert a model you trained yourself or otherwise trust.
"""

import argparse
import json
import pickle
import sys
from pathlib import Path


class UnsupportedModel(Exception):
    """The estimator cannot be represented in the exchange format yet."""


def convert_logistic_regression(model):
    """Map a binary LogisticRegression onto the exchange format."""
    coef = model.coef_
    intercept = model.intercept_

    if coef.shape[0] != 1:
        raise UnsupportedModel(
            f"only single output logistic regression is supported, this model has "
            f"{coef.shape[0]} coefficient rows (multi-class is not in the format yet)"
        )

    return {
        "kind": "logistic_regression",
        "weights": [float(w) for w in coef[0]],
        "bias": float(intercept[0]),
        # The format keeps the threshold explicit. scikit-learn predicts class 1
        # when the decision function is at or above zero, which is this value.
        "decision_threshold": 0.0,
    }


def convert_decision_tree(model):
    """Map a DecisionTreeClassifier onto the exchange format.

    scikit-learn stores the tree as parallel arrays where node 0 is the root
    and a node is a leaf when both children are -1. It sends a sample left when
    `feature <= threshold`, which is the same rule this project uses, so the
    indices carry over untouched.
    """
    tree = model.tree_

    if getattr(model, "n_outputs_", 1) != 1:
        raise UnsupportedModel(
            f"only single output trees are supported, this one has {model.n_outputs_}"
        )

    # The format stores a leaf as a number, so every class label has to be one.
    # Checked up front, so a model with text labels is refused with a message
    # instead of failing partway through the tree.
    leaf_values = []
    for label in model.classes_:
        try:
            leaf_values.append(float(label))
        except (TypeError, ValueError):
            raise UnsupportedModel(
                f"the tree's class labels are not numbers (found {str(label)!r} in "
                f"classes_), and the format stores a leaf as a number. Encode the "
                f"labels as integers before training, for example with "
                f"sklearn.preprocessing.LabelEncoder"
            ) from None

    nodes = []

    for i in range(tree.node_count):
        left = int(tree.children_left[i])
        right = int(tree.children_right[i])

        if left == -1 and right == -1:
            # The class this leaf predicts. Recent scikit-learn stores the
            # class proportions here, older releases stored raw counts, and
            # argmax picks the same class either way.
            proportions = tree.value[i][0]
            nodes.append({"type": "leaf", "value": leaf_values[int(proportions.argmax())]})
        else:
            nodes.append(
                {
                    "type": "split",
                    "feature_index": int(tree.feature[i]),
                    "threshold": float(tree.threshold[i]),
                    "left": left,
                    "right": right,
                }
            )

    return {
        "kind": "decision_tree",
        "num_features": int(tree.n_features),
        "nodes": nodes,
    }


CONVERTERS = {
    "LogisticRegression": convert_logistic_regression,
    "DecisionTreeClassifier": convert_decision_tree,
}


def convert(model):
    """Dispatch on the estimator class name."""
    name = type(model).__name__
    converter = CONVERTERS.get(name)

    if converter is None:
        supported = ", ".join(sorted(CONVERTERS))
        raise UnsupportedModel(f"{name} is not supported yet. Supported: {supported}")

    return converter(model)


def load(path):
    """Load a pickled estimator, preferring joblib when it is available."""
    try:
        import joblib

        return joblib.load(path)
    except ImportError:
        with open(path, "rb") as handle:
            return pickle.load(handle)


def main():
    parser = argparse.ArgumentParser(
        description="Convert a scikit-learn model into the JSON exchange format."
    )
    parser.add_argument("model", type=Path, help="pickled scikit-learn estimator")
    parser.add_argument(
        "-o", "--output", type=Path, required=True, help="where to write the JSON"
    )
    args = parser.parse_args()

    if not args.model.exists():
        print(f"Model not found: {args.model}", file=sys.stderr)
        return 1

    try:
        model = load(args.model)
    except Exception as error:
        print(f"Could not load {args.model}: {error}", file=sys.stderr)
        return 1

    try:
        payload = convert(model)
    except UnsupportedModel as error:
        print(f"Cannot convert this model: {error}", file=sys.stderr)
        return 1

    args.output.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"Wrote {payload['kind']} to {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
