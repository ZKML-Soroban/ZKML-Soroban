//! Model quantization utilities.
//!
//! Converts floating-point model parameters into fixed-point
//! representations suitable for arithmetic inside ZK circuits.

use zkml_common::fixed_point::FixedPoint;

/// Quantize a vector of `f64` values into fixed-point representations.
pub fn quantize_weights(weights: &[f64]) -> Vec<FixedPoint> {
    weights.iter().copied().map(FixedPoint::quantize).collect()
}

/// Quantize a single bias value.
pub fn quantize_bias(bias: f64) -> FixedPoint {
    FixedPoint::quantize(bias)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_preserves_sign() {
        let positive = quantize_bias(1.5);
        let negative = quantize_bias(-1.5);
        assert!(positive.value > 0);
        assert!(negative.value < 0);
    }

    #[test]
    fn quantize_vector() {
        let weights = vec![0.1, 0.2, 0.3];
        let quantized = quantize_weights(&weights);
        assert_eq!(quantized.len(), 3);
        for (orig, q) in weights.iter().zip(quantized.iter()) {
            assert!((orig - q.dequantize()).abs() < 1e-4);
        }
    }
}

/// Maximum absolute reconstruction error introduced by quantizing `values`.
///
/// Useful for asserting that a model survives quantization within tolerance.
pub fn max_quantization_error(values: &[f64]) -> f64 {
    values
        .iter()
        .map(|&v| (v - FixedPoint::quantize(v).dequantize()).abs())
        .fold(0.0, f64::max)
}

/// Dequantize a slice of fixed-point values back to `f64`.
pub fn dequantize_all(values: &[FixedPoint]) -> Vec<f64> {
    values.iter().map(FixedPoint::dequantize).collect()
}
