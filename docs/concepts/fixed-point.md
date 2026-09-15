---
title: "Fixed-point arithmetic"
description: "Q16.16 representation, checked operations, and determinism guarantees."
icon: "calculator"
---

ZK circuits operate over finite fields and cannot represent IEEE floats.
zkml-soroban maps weights, activations, and inputs to fixed-point
(`Q16.16` by default) so inference is a sequence of integer operations.

## Representation

A real value `x` is stored as `round(x * 2^scale)` in an `i64`, with
`scale = 16`. That gives about 4 to 5 decimal digits of fractional precision and
an integer range of roughly +/- 1.4e14.

```rust
use zkml_common::fixed_point::FixedPoint;

let w = FixedPoint::quantize(0.82);   // raw i64 = round(0.82 * 65536)
let x = FixedPoint::quantize(0.5);
let y = w.checked_mul(x).unwrap();    // i128 product, shift by 16
assert!((y.dequantize() - 0.41).abs() < 1e-4);
```

## Operations

| Operation           | Notes                                                                 |
| ------------------- | --------------------------------------------------------------------- |
| `checked_add`/`checked_sub` | Same-scale, `None` on overflow.                               |
| `checked_mul`       | Product in `i128`, shift right by `scale`, `None` if it exceeds `i64`. |
| `checked_div`       | Scaled division, `None` on division by zero or overflow.              |
| `saturating_add`    | Clamps to the representable range instead of overflowing.             |
| `+`, `-`, `*`       | Panicking wrappers over the checked variants.                          |
| `Neg`               | Plain negation of the raw value.                                       |
| `PartialOrd`/`Ord`  | Compare raw scaled integers; operands must share a scale.             |
| `abs`, `signum`, `is_zero`, `clamp` | Sign and range helpers.                                |
| `dot`               | Dot product with the same per-product rescaling as dense layers.      |

Reductions (`sum`, `mean`, `max`, `min`, `argmax`) and ZK-friendly activations
(`relu`, `leaky_relu` with power-of-two slope, `relu6`, `hard_sigmoid`,
`hard_swish`, `hardtanh`) are built on these primitives.

## Determinism

All operations are integer arithmetic with fixed rounding rules, so native
execution, the zkVM guest, and any future circuit agree bit for bit. Property
tests (`crates/zkml-common/tests/fixed_point_props.rs`) cover round trips,
commutativity, overflow detection, and ordering.

## Tie-breaking

`argmax` returns the lowest index among equal maxima, which makes the MLP
`class_label` deterministic.
