# Security Notes

## Determinism

Inference must be bit-for-bit identical between the native execution used for
testing and the in-circuit (or zkVM) execution used for proving. To guarantee
this:

- All arithmetic is fixed-point integer math; no floating-point appears in the
  inference path.
- Per-product rescaling in dense layers and dot products uses the same shift
  everywhere.
- Commitments fold field elements in a fixed, documented order.

## Overflow

Fixed-point multiplication uses an `i128` intermediate. The `checked_*` variants
surface overflow as `None` rather than wrapping silently, and dense-layer
accumulation bounds are documented in the devlog.

## Reviewing cryptographic code

Changes under `crates/zkml-verifier` and `commitment.rs` require maintainer
review. Unsafe code is rejected unless explicitly justified.
