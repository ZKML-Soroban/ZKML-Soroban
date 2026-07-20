//! Fixed-point arithmetic types for ZK-compatible model inference.
//!
//! ZK circuits operate over finite fields and cannot natively represent
//! floating-point numbers. This module provides fixed-point representations
//! that map model weights and activations into field-compatible integers.

use core::cmp::Ordering;
use core::ops::{Add, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

/// Number of fractional bits used in the fixed-point representation.
///
/// A scale of 16 means values are multiplied by 2^16 (65536) before
/// being stored as integers. This provides roughly 4-5 decimal digits
/// of precision, which is sufficient for most quantized ML models.
pub const DEFAULT_SCALE: u32 = 16;

/// A fixed-point number represented as a scaled integer.
///
/// Internally stores `value = round(real_value * 2^scale)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedPoint {
    /// The scaled integer value.
    pub value: i64,
    /// The number of fractional bits.
    pub scale: u32,
}

impl FixedPoint {
    /// Create a new fixed-point number from a raw scaled integer.
    pub fn from_raw(value: i64, scale: u32) -> Self {
        Self { value, scale }
    }

    /// Quantize a floating-point value into its fixed-point representation
    /// using the default scale.
    pub fn quantize(real: f64) -> Self {
        let factor = (1i64 << DEFAULT_SCALE) as f64;
        Self {
            value: (real * factor).round() as i64,
            scale: DEFAULT_SCALE,
        }
    }

    /// Reconstruct the approximate floating-point value.
    pub fn dequantize(&self) -> f64 {
        self.value as f64 / (1i64 << self.scale) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let original = 1.2345;
        let fp = FixedPoint::quantize(original);
        let recovered = fp.dequantize();
        assert!((original - recovered).abs() < 1e-4);
    }
}

impl FixedPoint {
    /// Checked addition of two fixed-point numbers with the same scale.
    ///
    /// Returns `None` on overflow. Both operands must share the same scale;
    /// this is checked with a debug assertion.
    pub fn checked_add(self, other: Self) -> Option<Self> {
        debug_assert_eq!(self.scale, other.scale, "scale mismatch in addition");
        self.value.checked_add(other.value).map(|value| Self {
            value,
            scale: self.scale,
        })
    }
}

#[cfg(test)]
mod tests_add {
    use super::*;

    #[test]
    fn add_matches_real_arithmetic() {
        let a = FixedPoint::quantize(1.25);
        let b = FixedPoint::quantize(2.50);
        let sum = a.checked_add(b).expect("no overflow");
        assert!((sum.dequantize() - 3.75).abs() < 1e-4);
    }

    #[test]
    fn add_overflow_returns_none() {
        let a = FixedPoint::from_raw(i64::MAX, DEFAULT_SCALE);
        let b = FixedPoint::from_raw(1, DEFAULT_SCALE);
        assert!(a.checked_add(b).is_none());
    }
}

impl FixedPoint {
    /// Checked subtraction of two fixed-point numbers with the same scale.
    ///
    /// Returns `None` on overflow. Both operands must share the same scale;
    /// this is checked with a debug assertion.
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        debug_assert_eq!(self.scale, other.scale, "scale mismatch in subtraction");
        self.value.checked_sub(other.value).map(|value| Self {
            value,
            scale: self.scale,
        })
    }
}

impl FixedPoint {
    /// Multiply two fixed-point numbers, rescaling the result.
    ///
    /// Uses an `i128` intermediate to avoid overflow before the shift back
    /// down by `scale` fractional bits.
    #[allow(clippy::should_implement_trait)]
    pub fn mul(self, other: Self) -> Self {
        debug_assert_eq!(self.scale, other.scale, "scale mismatch in multiply");
        let wide = (self.value as i128) * (other.value as i128);
        let scaled = wide >> self.scale;
        Self {
            value: scaled as i64,
            scale: self.scale,
        }
    }
}

impl FixedPoint {
    /// Saturating addition: clamps to the representable range on overflow.
    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            value: self.value.saturating_add(other.value),
            scale: self.scale,
        }
    }

    /// Negate the value.
    #[allow(clippy::should_implement_trait)]
    pub fn neg(self) -> Self {
        Self {
            value: -self.value,
            scale: self.scale,
        }
    }
}

#[cfg(test)]
mod tests_overflow {
    use super::*;

    #[test]
    fn saturating_add_clamps() {
        let a = FixedPoint::from_raw(i64::MAX, DEFAULT_SCALE);
        let b = FixedPoint::from_raw(1000, DEFAULT_SCALE);
        assert_eq!(a.saturating_add(b).value, i64::MAX);
    }

    #[test]
    fn mul_round_trips_small_values() {
        let a = FixedPoint::quantize(1.5);
        let b = FixedPoint::quantize(2.0);
        assert!((a.checked_mul(b).unwrap().dequantize() - 3.0).abs() < 1e-3);
    }
}

impl core::fmt::Display for FixedPoint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:.5}", self.dequantize())
    }
}

impl From<i32> for FixedPoint {
    fn from(value: i32) -> Self {
        FixedPoint::quantize(value as f64)
    }
}

impl FixedPoint {
    /// Checked multiplication of two fixed-point numbers with the same scale.
    ///
    /// Computes the product in an `i128` intermediate, shifts right by
    /// `scale` fractional bits, then returns `None` if the rescaled result
    /// does not fit in an `i64`. Both operands must share the same scale;
    /// this is checked with a debug assertion.
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        debug_assert_eq!(self.scale, other.scale, "scale mismatch in multiply");
        let wide = (self.value as i128) * (other.value as i128);
        let scaled = wide >> self.scale;
        i64::try_from(scaled).ok().map(|value| Self {
            value,
            scale: self.scale,
        })
    }
}

/// Compares the raw scaled integers, then scale as a tie-break.
///
/// Both operands must share the same scale; mismatched scales are a
/// programming error and are checked with a debug assertion. The scale
/// tie-break keeps this order consistent with the derived [`PartialEq`].
impl PartialOrd for FixedPoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Total order over raw scaled integers, then scale as a tie-break.
///
/// Same-scale comparison (the only supported case) orders by `value` alone.
/// When scales differ, `scale` breaks ties so `Ord` agrees with [`Eq`] even
/// in release builds where the debug assertion is compiled out.
impl Ord for FixedPoint {
    fn cmp(&self, other: &Self) -> Ordering {
        debug_assert_eq!(self.scale, other.scale, "scale mismatch in comparison");
        self.value
            .cmp(&other.value)
            .then(self.scale.cmp(&other.scale))
    }
}

/// Panicking addition. Prefer [`FixedPoint::checked_add`] when overflow is possible.
///
/// # Panics
///
/// Panics if the sum overflows `i64`.
impl Add for FixedPoint {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.checked_add(rhs).expect("FixedPoint addition overflow")
    }
}

/// Panicking subtraction. Prefer [`FixedPoint::checked_sub`] when overflow is possible.
///
/// # Panics
///
/// Panics if the difference overflows `i64`.
impl Sub for FixedPoint {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        self.checked_sub(rhs)
            .expect("FixedPoint subtraction overflow")
    }
}

/// Panicking multiplication with Q-format rescaling.
///
/// Prefer [`FixedPoint::checked_mul`] when overflow is possible. Uses an
/// `i128` intermediate, shifts right by `scale`, then panics if the result
/// does not fit in an `i64`.
///
/// # Panics
///
/// Panics if the rescaled product does not fit in an `i64`.
impl Mul for FixedPoint {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        self.checked_mul(rhs)
            .expect("FixedPoint multiplication overflow")
    }
}

/// Negates the fixed-point value (same scale).
impl Neg for FixedPoint {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            value: -self.value,
            scale: self.scale,
        }
    }
}

#[cfg(test)]
mod tests_mul {
    use super::*;

    #[test]
    fn checked_mul_handles_large_values() {
        let big = FixedPoint::quantize(20_000_000.0);
        // (2e7)^2 rescaled by 2^16 exceeds i64::MAX, so checked_mul -> None.
        assert!(big.checked_mul(big).is_none());
    }

    #[test]
    fn checked_mul_small_values_ok() {
        let a = FixedPoint::quantize(2.0);
        let b = FixedPoint::quantize(3.0);
        assert!((a.checked_mul(b).unwrap().dequantize() - 6.0).abs() < 1e-3);
    }

    #[test]
    fn checked_mul_scale_correctness_two_point_five_times_four() {
        // Acceptance criterion from issue #2: 2.5 * 4.0 == 10.0 in Q16.16.
        let a = FixedPoint::quantize(2.5);
        let b = FixedPoint::quantize(4.0);
        let product = a.checked_mul(b).expect("no overflow");
        assert!((product.dequantize() - 10.0).abs() < 1e-3);
    }
}

#[cfg(test)]
mod tests_arithmetic_ops {
    use super::*;

    #[test]
    fn additive_identity() {
        let a = FixedPoint::quantize(3.25);
        let zero = FixedPoint::from_raw(0, DEFAULT_SCALE);
        let sum = a.checked_add(zero).expect("no overflow");
        assert_eq!(sum.value, a.value);
        assert_eq!((a + zero).value, a.value);
    }

    #[test]
    fn add_sub_round_trip() {
        let a = FixedPoint::quantize(7.5);
        let b = FixedPoint::quantize(2.25);
        let back = (a.checked_add(b).expect("add") - b).dequantize();
        assert!((back - 7.5).abs() < 1e-3);
    }

    #[test]
    fn negative_add_and_sub() {
        let a = FixedPoint::quantize(-1.5);
        let b = FixedPoint::quantize(2.0);
        assert!((a.checked_add(b).unwrap().dequantize() - 0.5).abs() < 1e-4);
        assert!(
            (FixedPoint::quantize(1.0)
                .checked_sub(FixedPoint::quantize(2.5))
                .unwrap()
                .dequantize()
                + 1.5)
                .abs()
                < 1e-4
        );
    }

    #[test]
    fn negative_mul() {
        let a = FixedPoint::quantize(-2.0);
        let b = FixedPoint::quantize(3.0);
        assert!((a.checked_mul(b).unwrap().dequantize() + 6.0).abs() < 1e-3);
        assert!(
            (a.checked_mul(FixedPoint::quantize(-3.0))
                .unwrap()
                .dequantize()
                - 6.0)
                .abs()
                < 1e-3
        );
    }

    #[test]
    fn sub_overflow_returns_none() {
        let a = FixedPoint::from_raw(i64::MIN, DEFAULT_SCALE);
        let b = FixedPoint::from_raw(1, DEFAULT_SCALE);
        assert!(a.checked_sub(b).is_none());
    }

    #[test]
    fn operators_match_checked_variants() {
        let a = FixedPoint::quantize(1.5);
        let b = FixedPoint::quantize(2.5);
        assert_eq!((a + b).value, a.checked_add(b).unwrap().value);
        assert_eq!((a - b).value, a.checked_sub(b).unwrap().value);
        assert_eq!((a * b).value, a.checked_mul(b).unwrap().value);
        assert_eq!((-a).value, -a.value);
    }

    #[test]
    fn ordering_compares_raw_values() {
        let neg = FixedPoint::quantize(-1.0);
        let zero = FixedPoint::from_raw(0, DEFAULT_SCALE);
        let pos = FixedPoint::quantize(2.0);
        assert!(neg < zero);
        assert!(zero < pos);
        assert!(neg < pos);
        assert!(pos.cmp(&pos).is_eq());
        assert_eq!(pos.cmp(&neg), Ordering::Greater);
        assert!(pos > neg);
    }

    #[test]
    fn ord_agrees_with_eq() {
        // Ord's contract: a.cmp(b) == Equal iff a == b (derived PartialEq/Eq).
        let a = FixedPoint::from_raw(100, DEFAULT_SCALE);
        let b = FixedPoint::from_raw(100, DEFAULT_SCALE);
        let c = FixedPoint::from_raw(50, DEFAULT_SCALE);
        assert_eq!(a, b);
        assert!(a.cmp(&b).is_eq());
        assert_ne!(a, c);
        assert!(!a.cmp(&c).is_eq());
        assert_eq!(a.cmp(&c), c.cmp(&a).reverse());

        // Same raw value, different scale: PartialEq is false. Ord keeps a
        // debug_assert on scale mismatch (panics in debug), and uses scale as
        // a tie-break so release builds also report non-Equal for this pair.
        let d = FixedPoint::from_raw(100, 8);
        assert_ne!(a, d);
        assert_ne!(a.scale, d.scale);
    }

    /// In release, `debug_assert` is compiled out; the scale tie-break must
    /// keep `Ord` consistent with `PartialEq` for mismatched scales.
    #[test]
    #[cfg(not(debug_assertions))]
    fn ord_scale_tie_break_matches_eq_in_release() {
        let a = FixedPoint::from_raw(100, 16);
        let b = FixedPoint::from_raw(100, 8);
        assert_ne!(a, b);
        assert!(!a.cmp(&b).is_eq());
        assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
    }
}

impl FixedPoint {
    /// Divide two fixed-point numbers, returning `None` on divide-by-zero.
    pub fn checked_div(self, other: Self) -> Option<Self> {
        debug_assert_eq!(self.scale, other.scale, "scale mismatch in division");
        if other.value == 0 {
            return None;
        }
        let wide = ((self.value as i128) << self.scale) / (other.value as i128);
        i64::try_from(wide).ok().map(|value| Self {
            value,
            scale: self.scale,
        })
    }
}

#[cfg(test)]
mod tests_div {
    use super::*;

    #[test]
    fn div_round_trips() {
        let a = FixedPoint::quantize(6.0);
        let b = FixedPoint::quantize(2.0);
        assert!((a.checked_div(b).unwrap().dequantize() - 3.0).abs() < 1e-3);
    }

    #[test]
    fn div_by_zero_is_none() {
        let a = FixedPoint::quantize(1.0);
        let zero = FixedPoint::from_raw(0, DEFAULT_SCALE);
        assert!(a.checked_div(zero).is_none());
    }
}

impl FixedPoint {
    /// Absolute value. Saturates `i64::MIN` to `i64::MAX` to stay in range.
    pub fn abs(self) -> Self {
        Self {
            value: self.value.saturating_abs(),
            scale: self.scale,
        }
    }

    /// Returns `true` if the value is exactly zero.
    pub fn is_zero(self) -> bool {
        self.value == 0
    }

    /// Clamp the value into the inclusive range `[min, max]`.
    ///
    /// All three operands must share the same scale (checked in debug builds).
    pub fn clamp(self, min: Self, max: Self) -> Self {
        debug_assert_eq!(self.scale, min.scale, "scale mismatch in clamp");
        debug_assert_eq!(self.scale, max.scale, "scale mismatch in clamp");
        Self {
            value: self.value.clamp(min.value, max.value),
            scale: self.scale,
        }
    }

    /// Sign of the value: `-1`, `0`, or `1`.
    pub fn signum(self) -> i64 {
        self.value.signum()
    }
}

#[cfg(test)]
mod tests_sign {
    use super::*;

    #[test]
    fn abs_of_negative_is_positive() {
        let n = FixedPoint::quantize(-3.5);
        assert!((n.abs().dequantize() - 3.5).abs() < 1e-4);
    }

    #[test]
    fn abs_saturates_min() {
        let n = FixedPoint::from_raw(i64::MIN, DEFAULT_SCALE);
        assert_eq!(n.abs().value, i64::MAX);
    }

    #[test]
    fn is_zero_detects_zero() {
        assert!(FixedPoint::from_raw(0, DEFAULT_SCALE).is_zero());
        assert!(!FixedPoint::quantize(0.1).is_zero());
    }

    #[test]
    fn signum_reports_sign() {
        assert_eq!(FixedPoint::quantize(2.0).signum(), 1);
        assert_eq!(FixedPoint::quantize(-2.0).signum(), -1);
        assert_eq!(FixedPoint::from_raw(0, DEFAULT_SCALE).signum(), 0);
    }

    #[test]
    fn clamp_bounds_value() {
        let lo = FixedPoint::quantize(-1.0);
        let hi = FixedPoint::quantize(1.0);
        assert!((FixedPoint::quantize(5.0).clamp(lo, hi).dequantize() - 1.0).abs() < 1e-4);
        assert!((FixedPoint::quantize(-5.0).clamp(lo, hi).dequantize() + 1.0).abs() < 1e-4);
        assert!((FixedPoint::quantize(0.5).clamp(lo, hi).dequantize() - 0.5).abs() < 1e-4);
    }
}

/// Fixed-point dot product of two equal-length vectors.
///
/// Panics in debug builds if the lengths differ.
pub fn dot(a: &[FixedPoint], b: &[FixedPoint]) -> FixedPoint {
    debug_assert_eq!(a.len(), b.len(), "dot product length mismatch");
    let scale = a.first().map(|x| x.scale).unwrap_or(DEFAULT_SCALE);
    let acc: i64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (x.value * y.value) >> scale)
        .sum();
    FixedPoint::from_raw(acc, scale)
}

#[cfg(test)]
mod tests_dot {
    use super::*;

    #[test]
    fn dot_matches_manual() {
        let a = vec![FixedPoint::quantize(1.0), FixedPoint::quantize(2.0)];
        let b = vec![FixedPoint::quantize(3.0), FixedPoint::quantize(4.0)];
        // 1*3 + 2*4 = 11
        assert!((dot(&a, &b).dequantize() - 11.0).abs() < 1e-2);
    }
}

/// Sum of a slice of fixed-point values.
///
/// Returns zero at the default scale for an empty slice.
pub fn sum(xs: &[FixedPoint]) -> FixedPoint {
    let scale = xs.first().map(|x| x.scale).unwrap_or(DEFAULT_SCALE);
    let acc: i64 = xs.iter().map(|x| x.value).sum();
    FixedPoint::from_raw(acc, scale)
}

/// Arithmetic mean of a slice of fixed-point values.
///
/// Returns `None` for an empty slice (no meaningful average).
pub fn mean(xs: &[FixedPoint]) -> Option<FixedPoint> {
    if xs.is_empty() {
        return None;
    }
    let total = sum(xs);
    Some(FixedPoint::from_raw(
        total.value / xs.len() as i64,
        total.scale,
    ))
}

/// Largest value in a slice.
///
/// Returns `None` for an empty slice. Used by max-pooling layers.
pub fn max(xs: &[FixedPoint]) -> Option<FixedPoint> {
    xs.iter()
        .copied()
        .reduce(|a, b| if b.value > a.value { b } else { a })
}

/// Smallest value in a slice.
///
/// Returns `None` for an empty slice.
pub fn min(xs: &[FixedPoint]) -> Option<FixedPoint> {
    xs.iter()
        .copied()
        .reduce(|a, b| if b.value < a.value { b } else { a })
}

/// Index of the largest value in a slice.
///
/// Returns `None` for an empty slice. On ties the lowest index wins, which
/// matches the usual argmax convention for classification outputs.
pub fn argmax(xs: &[FixedPoint]) -> Option<usize> {
    let mut best: Option<(usize, i64)> = None;
    for (i, x) in xs.iter().enumerate() {
        if best.is_none_or(|(_, v)| x.value > v) {
            best = Some((i, x.value));
        }
    }
    best.map(|(i, _)| i)
}

#[cfg(test)]
mod tests_reduce {
    use super::*;

    #[test]
    fn sum_adds_values() {
        let xs = vec![
            FixedPoint::quantize(1.0),
            FixedPoint::quantize(2.0),
            FixedPoint::quantize(3.0),
        ];
        assert!((sum(&xs).dequantize() - 6.0).abs() < 1e-3);
    }

    #[test]
    fn mean_averages_values() {
        let xs = vec![FixedPoint::quantize(2.0), FixedPoint::quantize(4.0)];
        assert!((mean(&xs).unwrap().dequantize() - 3.0).abs() < 1e-3);
    }

    #[test]
    fn mean_of_empty_is_none() {
        assert!(mean(&[]).is_none());
    }

    #[test]
    fn max_picks_largest() {
        let xs = vec![
            FixedPoint::quantize(-1.0),
            FixedPoint::quantize(3.5),
            FixedPoint::quantize(2.0),
        ];
        assert!((max(&xs).unwrap().dequantize() - 3.5).abs() < 1e-3);
    }

    #[test]
    fn min_picks_smallest() {
        let xs = vec![
            FixedPoint::quantize(-1.0),
            FixedPoint::quantize(3.5),
            FixedPoint::quantize(2.0),
        ];
        assert!((min(&xs).unwrap().dequantize() + 1.0).abs() < 1e-3);
    }

    #[test]
    fn max_and_min_of_empty_are_none() {
        assert!(max(&[]).is_none());
        assert!(min(&[]).is_none());
    }

    #[test]
    fn argmax_reports_index() {
        let xs = vec![
            FixedPoint::quantize(0.1),
            FixedPoint::quantize(0.7),
            FixedPoint::quantize(0.2),
        ];
        assert_eq!(argmax(&xs), Some(1));
    }

    #[test]
    fn argmax_breaks_ties_low() {
        let xs = vec![FixedPoint::quantize(1.0), FixedPoint::quantize(1.0)];
        assert_eq!(argmax(&xs), Some(0));
    }

    #[test]
    fn argmax_of_empty_is_none() {
        assert!(argmax(&[]).is_none());
    }
}
