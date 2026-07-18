# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `PartialOrd` / `Ord` for `FixedPoint` (same-scale raw integer comparison).
- `Add`, `Sub`, and `Mul` operators as panicking wrappers over checked arithmetic.
- `Neg` for `FixedPoint` so negation uses the standard unary operator.
- `FixedPoint::abs`, `is_zero`, and `signum` helpers.
- `FixedPoint::clamp` for range saturation.
- `sum` and `mean` slice reductions for pooling layers.
- `max`, `min`, and `argmax` slice reductions for max-pooling and classification.
- ZK-friendly `leaky_relu` activation with a power-of-two slope.
- `relu6` bounded activation for quantized networks.
- `hard_sigmoid` and `hard_swish` piecewise-linear activations.
- `hardtanh` bounded activation clamping to `[-1, 1]`.
- Element-wise `relu6_vec`, `hard_sigmoid_vec`, and `hard_swish_vec` helpers.
- Initial workspace scaffold with `zkml-common`, `zkml-prover`, and
  `zkml-verifier` crates.

[Unreleased]: https://github.com/diegoveme/ZKML-Soroban/compare/main...HEAD

## [0.2.0] - 2026-06-17

### Added
- Fixed-point checked/saturating arithmetic, division, and dot product.
- Quantized ReLU activation, `Tensor` type, and model validation helpers.
- Model and input commitments plus a Merkle tree over parameters.
- JSON model import, batch inference, and validated inference.
- A prover CLI binary printing the model commitment and output.
- Verification bundle assembly, JSON serialization, and bundle ids.
- Verifier contract: public input parsing, events, and query methods.
- Documentation set, CI with fmt/clippy, and contributor tooling.

[0.2.0]: https://github.com/diegoveme/ZKML-Soroban/releases/tag/v0.2.0
