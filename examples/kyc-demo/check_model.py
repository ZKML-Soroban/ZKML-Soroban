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

CORE_DOMAINS = ("", "ai.onnx")


def core_opset(model):
    """Return the lowest core domain opset import, or None if there is none.

    A model can declare the core domain more than once, as "" and as
    "ai.onnx". The importer checks every entry, so the lowest one is what
    decides whether the file is accepted.
    """
    versions = [e.version for e in model.opset_import if e.domain in CORE_DOMAINS]
    return min(versions) if versions else None


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

    version = core_opset(model)

    if version is None:
        print("\nNo core opset import found. The importer treats that as malformed.")
        return 1

    if version < MIN_OPSET_CORE:
        print(
            f"\nCore opset is {version}, the importer requires {MIN_OPSET_CORE} or higher."
        )
        print("Re-export with target_opset=17 in train_model.py.")
        return 1

    print(f"\nCore opset {version} meets the minimum of {MIN_OPSET_CORE}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
