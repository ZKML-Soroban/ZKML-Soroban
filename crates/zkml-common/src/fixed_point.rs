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
        let original = 3.14159;
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
        self.value
            .checked_add(other.value)
            .map(|value| Self { value, scale: self.scale })
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
        self.value
            .checked_sub(other.value)
            .map(|value| Self { value, scale: self.scale })
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
        Self { value: scaled as i64, scale: self.scale }
    }
}
