#!/usr/bin/env python3
"""
Inspect the exported ONNX model before importing it with zkml-prover.

Prints every opset import and every operator in the graph, and fails when the
core domain is below the floor the importer enforces (MIN_OPSET_CORE in
crates/zkml-prover/src/onnx/validate.rs).
"""

import sys
from pathlib import Path

import onnx

# Keep in sync with MIN_OPSET_CORE and MIN_OPSET_ML in
# crates/zkml-prover/src/onnx/validate.rs.
MIN_OPSET_CORE = 17
MIN_OPSET_ML = 1

# An empty domain string is the core domain, same as "ai.onnx".
FLOORS = {"": MIN_OPSET_CORE, "ai.onnx": MIN_OPSET_CORE, "ai.onnx.ml": MIN_OPSET_ML}


def check_opsets(model):
    """Mirror `check_opset` in validate.rs and return the reason it would fail.

    The importer rejects a model when it declares no opset imports at all, when
    a domain it knows sits below its floor, or when none of the declared
    domains is one it knows.
    """
    if not model.opset_import:
        return "the model declares no opset_import entries"

    saw_known = False

    for entry in model.opset_import:
        floor = FLOORS.get(entry.domain)

        if floor is None:
            continue

        saw_known = True

        if entry.version < floor:
            domain = entry.domain or "the core domain"
            return (
                f"{domain} is at opset {entry.version}, the importer requires "
                f"{floor} or higher. Re-export with target_opset=17 in "
                f"train_model.py"
            )

    if not saw_known:
        return "the model has no opset_import for ai.onnx or ai.onnx.ml"

    return None


def main():
    model_path = Path(__file__).parent / "kyc_decision_tree.onnx"

    if len(sys.argv) > 1:
        model_path = Path(sys.argv[1])

    if not model_path.exists():
        print(f"Model not found: {model_path}")
        print("Run generate_dataset.py and train_model.py first.")
        return 1

    model = onnx.load(str(model_path))

    print(f"Model: {model_path}")
    print("\nOpset imports:")
    for entry in model.opset_import:
        domain = entry.domain or "(core)"
        print(f"  {domain}: {entry.version}")

    print("\nOperators:")
    for node in model.graph.node:
        domain = node.domain or "(core)"
        print(f"  {node.op_type} [{domain}]")

    problem = check_opsets(model)

    if problem is not None:
        print(f"\nThis model would be rejected: {problem}.")
        return 1

    print("\nEvery declared opset meets the floor the importer enforces.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
