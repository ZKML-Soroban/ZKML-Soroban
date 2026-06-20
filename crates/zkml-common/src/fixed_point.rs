//! Fixed-point arithmetic types for ZK-compatible model inference.
//!
//! ZK circuits operate over finite fields and cannot natively represent
//! floating-point numbers. This module provides fixed-point representations
//! that map model weights and activations into field-compatible integers.

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
        assert!((a.mul(b).dequantize() - 3.0).abs() < 1e-3);
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
    /// Checked multiplication, returning `None` if the rescaled result does
    /// not fit in an `i64`.
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
