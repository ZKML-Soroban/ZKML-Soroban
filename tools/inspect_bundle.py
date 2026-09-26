#!/usr/bin/env python3
"""
Read a verification bundle and print what the contract receives.

    python tools/inspect_bundle.py bundle.json
    python tools/inspect_bundle.py bundle.json --check

The on-chain public input is one 80-byte blob, laid out as
`model_hash (32) || input_hash (32) || output (8) || class_label (8)`, with the
two integers little-endian. That layout is documented in
docs/reference/verifier-contract.md.

This reads the bundle `zkml-prover prove` writes. A v2 bundle carries a RISC
Zero journal instead and is checked with `zkml-prover verify-bundle`.
"""

import argparse
import json
import struct
import sys
from pathlib import Path

HASH_LEN = 32
INT_LEN = 8
BLOB_LEN = 80
SCALE_FACTOR = 1 << 16


class MalformedBundle(Exception):
    """The bundle cannot be read as the contract would read it."""


def read_public_inputs(bundle):
    """Pull the four fields out of the bundle, or explain what is missing."""
    if "version" in bundle and "journal" in bundle:
        raise MalformedBundle(
            "this is a v2 bundle: its public inputs live in the RISC Zero "
            "journal. Use `zkml-prover verify-bundle` for those."
        )

    public_inputs = bundle.get("public_inputs")

    if public_inputs is None:
        raise MalformedBundle("no `public_inputs` field")

    missing = [
        key
        for key in ("model_hash", "input_hash", "output", "class_label")
        if key not in public_inputs
    ]

    if missing:
        raise MalformedBundle(f"`public_inputs` is missing {', '.join(missing)}")

    return public_inputs


def validate(public_inputs):
    """Return the problems with the field sizes, empty when there are none."""
    problems = []

    for name, expected in (
        ("model_hash", HASH_LEN),
        ("input_hash", HASH_LEN),
        ("output", INT_LEN),
    ):
        value = public_inputs[name]

        if not isinstance(value, list):
            problems.append(
                f"{name} should be a byte array, found {type(value).__name__}"
            )
            continue

        if len(value) != expected:
            problems.append(f"{name} should be {expected} bytes, found {len(value)}")
            continue

        if any(not isinstance(b, int) or not 0 <= b <= 255 for b in value):
            problems.append(f"{name} holds something that is not a byte")

    label = public_inputs["class_label"]

    if not isinstance(label, int):
        problems.append(
            f"class_label should be an integer, found {type(label).__name__}"
        )
    elif not -(2**63) <= label < 2**63:
        problems.append(f"class_label does not fit in an i64: {label}")

    return problems


def build_blob(public_inputs):
    """Rebuild the 80-byte public input in the order the contract reads it."""
    return (
        bytes(public_inputs["model_hash"])
        + bytes(public_inputs["input_hash"])
        + bytes(public_inputs["output"])
        + struct.pack("<q", public_inputs["class_label"])
    )


def main():
    parser = argparse.ArgumentParser(
        description="Print the public inputs a verification bundle carries."
    )
    parser.add_argument(
        "bundle", type=Path, help="bundle written by `zkml-prover prove`"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate the field sizes and exit non-zero when something is off",
    )
    args = parser.parse_args()

    if not args.bundle.exists():
        print(f"Bundle not found: {args.bundle}", file=sys.stderr)
        return 1

    try:
        bundle = json.loads(args.bundle.read_text())
    except json.JSONDecodeError as error:
        print(f"Not valid JSON: {error}", file=sys.stderr)
        return 1

    try:
        public_inputs = read_public_inputs(bundle)
    except MalformedBundle as error:
        print(f"Cannot read this bundle: {error}", file=sys.stderr)
        return 1

    problems = validate(public_inputs)

    if problems:
        print(f"Bundle: {args.bundle}")
        for problem in problems:
            print(f"  problem: {problem}", file=sys.stderr)
        return 1

    blob = build_blob(public_inputs)
    output_raw = struct.unpack("<q", bytes(public_inputs["output"]))[0]

    print(f"Bundle: {args.bundle}")
    print(f"  model_hash  : {bytes(public_inputs['model_hash']).hex()}")
    print(f"  input_hash  : {bytes(public_inputs['input_hash']).hex()}")
    print(f"  output      : {output_raw} raw Q16.16, {output_raw / SCALE_FACTOR}")
    print(f"  class_label : {public_inputs['class_label']}")
    print(f"  public input ({len(blob)} bytes):")
    print(f"    {blob.hex()}")

    if args.check:
        if len(blob) != BLOB_LEN:
            print(
                f"  problem: the public input is {len(blob)} bytes, "
                f"the contract reads {BLOB_LEN}",
                file=sys.stderr,
            )
            return 1
        print(f"  check: {BLOB_LEN} bytes, every field the right size")

    return 0


if __name__ == "__main__":
    sys.exit(main())
