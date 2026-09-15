---
title: "Security notes"
description: "Determinism, overflow handling, and review rules for consensus-critical code."
icon: "lock"
---

## Determinism

Inference must be bit-for-bit identical between native execution and the zkVM
guest. To guarantee this:

- No floating-point arithmetic appears in the inference path.
- Host and guest call the same `zkml_common::inference` functions.
- Dense layers and dot products use the same per-product rescaling everywhere.
- Commitments fold elements in a fixed, documented order.
- `argmax` breaks ties by lowest index.
- The host cross-checks every journal field against native inference.

## Overflow

- Fixed-point multiplication uses an `i128` intermediate; `checked_*` variants
  return `None` instead of wrapping.
- Logistic regression and dense layers use `checked_mul` for every product and
  `checked_add` for the accumulator, surfacing overflow as an error.
- `zkml-prover validate` runs static overflow bounds for a bounded input
  magnitude before a model is deployed.
- The contract is built with `overflow-checks = true` in the `contract` profile.

## Decision tree termination

`DecisionTree::validate` rejects out-of-range children, self references, cycles,
and unreachable nodes. Traversal also has an iteration bound, so malformed trees
return an error instead of looping.

## Contract hygiene

- Public input lengths are validated before any curve operation on them.
- Nullifiers are stored only after the pairing check passes.
- Failed verifications do not bump TTL or change state.

## Consensus-critical code

Changes to these areas change proofs or commitments and require maintainer
review plus updated snapshots or golden vectors:

- `crates/zkml-verifier/src/lib.rs`
- `crates/zkml-common/src/commitment.rs` and `merkle.rs`
- `crates/zkml-common/src/fixed_point.rs` and `inference.rs`
- `methods/guest/src/main.rs`

Unsafe code is rejected unless explicitly justified.

## Reporting vulnerabilities

Follow the process in
[SECURITY.md](https://github.com/ZKML-Soroban/ZKML-Soroban/blob/main/SECURITY.md).
Do not open public issues for vulnerabilities.
