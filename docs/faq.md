---
title: "FAQ"
description: "Common questions about design choices and current capabilities."
icon: "circle-question"
---

## Why fixed-point instead of floating-point?

ZK circuits operate over finite fields and cannot represent IEEE floats.
Fixed-point keeps every operation an exact integer computation that native
code, the zkVM, and a circuit evaluate identically.

## Why is the sigmoid omitted in logistic regression?

The sigmoid is non-linear and expensive to constrain. Comparing the raw linear
score with a threshold yields the same binary decision, which is what
`class_label` records.

## Why RISC Zero before native circuits?

RISC Zero proves ordinary Rust, so the same inference engine runs natively and
in the zkVM without hand-written circuits. Native BN254 circuits (Phase 2) are
the optimization for smaller proofs and faster proving.

## Can I verify a real proof on testnet today?

Not yet. The contract's Groth16 verification is implemented and tested with a
valid pairing fixture, but the prover does not yet produce Groth16 proofs (the
STARK-to-Groth16 wrap and verification key export are pending). See
[Known limitations](/security/known-limitations).

## What model sizes are supported?

Small models: single decision trees, binary logistic regression, and tiny MLPs
with ReLU. Proving cost grows with the computation, so large networks are out of
scope.

## Are the model weights and inputs private?

They are not published on-chain: only their Poseidon commitments, the output,
and the class label appear in the public inputs. Whoever runs the prover sees
the model and the inputs.

## Can the same result be verified twice?

No. The contract stores a nullifier derived from the public inputs, so an
identical `(model, input, output, class_label)` tuple returns
`ProofAlreadyUsed` on resubmission.

## Which Stellar protocol version is required?

Protocol 25 or later, for the BN254 host functions (CAP-0074).
