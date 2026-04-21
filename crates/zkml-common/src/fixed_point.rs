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
