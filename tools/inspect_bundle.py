#!/usr/bin/env python3
"""
Read a verification bundle and print what the contract receives.

    python tools/inspect_bundle.py bundle.json
    python tools/inspect_bundle.py bundle.json --check

The on-chain public input is one 80-byte blob, laid out as
`model_hash (32) || input_hash (32) || output (8) || class_label (8)`, with the
two integers little-endian. That layout is documented in
docs/reference/verifier-contract.md.

Every field is printed on its own, including a malformed one, so the tool shows
what is inside a broken bundle rather than only that it is broken. Problems are
listed next to the field they belong to. Without `--check` the exit code is 0
either way; with it, any problem makes the exit code 1.

A file that cannot be read as a bundle at all, because it is not JSON, has no
`public_inputs`, or is a v2 bundle, always exits with 1: there is nothing to
show.

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
FIELDS = ("model_hash", "input_hash", "output", "class_label")


class UnreadableBundle(Exception):
    """The file cannot be read as a v1 bundle, so there is nothing to show."""


def read_public_inputs(bundle):
    """Return the `public_inputs` object, or explain why there is none."""
    if not isinstance(bundle, dict):
        raise UnreadableBundle("the file holds JSON, but not an object")

    if "version" in bundle and "journal" in bundle:
        raise UnreadableBundle(
            "this is a v2 bundle: its public inputs live in the RISC Zero "
            "journal. Use `zkml-prover verify-bundle` for those."
        )

    public_inputs = bundle.get("public_inputs")

    if not isinstance(public_inputs, dict):
        raise UnreadableBundle("no `public_inputs` object")

    return public_inputs


def read_bytes(value, expected):
    """Return (hex or None, problem or None) for a byte array field."""
    if not isinstance(value, list):
        return None, f"should be a byte array, found {type(value).__name__}"

    if any(not isinstance(b, int) or not 0 <= b <= 255 for b in value):
        return None, "holds something that is not a byte"

    shown = bytes(value).hex()

    if len(value) != expected:
        return shown, f"should be {expected} bytes, found {len(value)}"

    return shown, None


def describe(public_inputs):
    """Return one (name, text, problem) row per field, in layout order."""
    rows = []

    for name in ("model_hash", "input_hash"):
        if name not in public_inputs:
            rows.append((name, "missing", f"{name} is missing"))
            continue
        shown, problem = read_bytes(public_inputs[name], HASH_LEN)
        rows.append((name, shown or "unreadable", problem and f"{name} {problem}"))

    if "output" not in public_inputs:
        rows.append(("output", "missing", "output is missing"))
    else:
        shown, problem = read_bytes(public_inputs["output"], INT_LEN)
        if problem is None:
            raw = struct.unpack("<q", bytes(public_inputs["output"]))[0]
            shown = f"{raw} raw Q16.16, {raw / SCALE_FACTOR}"
        rows.append(("output", shown or "unreadable", problem and f"output {problem}"))

    if "class_label" not in public_inputs:
        rows.append(("class_label", "missing", "class_label is missing"))
    else:
        label = public_inputs["class_label"]
        if isinstance(label, bool) or not isinstance(label, int):
            rows.append(
                (
                    "class_label",
                    repr(label),
                    f"class_label should be an integer, found {type(label).__name__}",
                )
            )
        elif not -(2**63) <= label < 2**63:
            rows.append(
                ("class_label", str(label), f"class_label does not fit in an i64: {label}")
            )
        else:
            rows.append(("class_label", str(label), None))

    return rows


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
        help="exit with 1 when any field is malformed; without it, problems are "
        "reported and the exit code is 0",
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
    except UnreadableBundle as error:
        print(f"Cannot read this bundle: {error}", file=sys.stderr)
        return 1

    rows = describe(public_inputs)
    problems = [problem for _, _, problem in rows if problem]

    print(f"Bundle: {args.bundle}")
    for name, text, problem in rows:
        print(f"  {name:<12}: {text}")
        if problem:
            print(f"    problem: {problem}")

    if problems:
        count = len(problems)
        print(f"  public input: not built, {count} problem{'s' if count > 1 else ''} above")
        if args.check:
            print("  check: failed", file=sys.stderr)
            return 1
        return 0

    blob = build_blob(public_inputs)
    print(f"  public input ({len(blob)} bytes):")
    print(f"    {blob.hex()}")

    if args.check:
        print(f"  check: {BLOB_LEN} bytes, every field the right size")

    return 0


if __name__ == "__main__":
    sys.exit(main())
