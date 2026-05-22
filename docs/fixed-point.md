# Fixed-Point Arithmetic

ZK circuits operate over finite fields and cannot represent floating-point
numbers natively. zkml-soroban maps all model weights, activations, and inputs
into a fixed-point representation (`Q16.16` by default) so that inference is a
sequence of integer operations that a circuit can constrain.

## Representation

A real value `x` is stored as `round(x * 2^scale)` with `scale = 16`. This
yields roughly 4 to 5 decimal digits of precision.

## Operations

| Operation      | Notes                                                        |
| -------------- | ------------------------------------------------------------ |
| `checked_add`  | Same-scale addition, `None` on overflow.                     |
| `checked_sub`  | Same-scale subtraction, `None` on overflow.                  |
| `mul`          | Uses an `i128` intermediate, then shifts right by `scale`.   |
| `saturating_add` | Clamps to the representable range instead of overflowing.  |

## Determinism

All operations are deterministic integer arithmetic. This matters because the
off-chain prover and any future in-circuit evaluation must agree bit for bit.

## Vector helpers

`dot(a, b)` computes a fixed-point dot product with the same per-product
rescaling used inside dense layers, so the prover and any in-circuit evaluation
agree on intermediate values.
