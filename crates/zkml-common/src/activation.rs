//! Activation functions for quantized neural network layers.
//!
//! Only ZK-friendly activations are supported. ReLU is the workhorse because
//! it reduces to a comparison and a select, both cheap to constrain.

use crate::fixed_point::FixedPoint;

/// Quantized ReLU: `max(0, x)`.
pub fn relu(x: FixedPoint) -> FixedPoint {
    if x.value < 0 {
        FixedPoint::from_raw(0, x.scale)
    } else {
        x
    }
}

/// Apply ReLU element-wise to a slice, returning a new vector.
pub fn relu_vec(xs: &[FixedPoint]) -> Vec<FixedPoint> {
    xs.iter().copied().map(relu).collect()
}

/// Quantized leaky ReLU: passes positives through and scales negatives by a
/// small slope instead of zeroing them.
///
/// The slope is expressed as a power-of-two right shift so it stays a cheap
/// constraint in the circuit (a shift, not a general multiply). `shift = 4`
/// approximates a slope of 1/16 (0.0625), within the common 0.01-0.1 range.
pub fn leaky_relu(x: FixedPoint, shift: u32) -> FixedPoint {
    if x.value < 0 {
        FixedPoint::from_raw(x.value >> shift, x.scale)
    } else {
        x
    }
}

/// Apply leaky ReLU element-wise to a slice, returning a new vector.
pub fn leaky_relu_vec(xs: &[FixedPoint], shift: u32) -> Vec<FixedPoint> {
    xs.iter().copied().map(|x| leaky_relu(x, shift)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relu_clamps_negatives() {
        let neg = FixedPoint::quantize(-2.0);
        assert_eq!(relu(neg).value, 0);
    }

    #[test]
    fn relu_passes_positives() {
        let pos = FixedPoint::quantize(2.0);
        assert_eq!(relu(pos).value, pos.value);
    }

    #[test]
    fn leaky_relu_keeps_positives() {
        let pos = FixedPoint::quantize(2.0);
        assert_eq!(leaky_relu(pos, 4).value, pos.value);
    }

    #[test]
    fn leaky_relu_dampens_negatives() {
        let neg = FixedPoint::quantize(-1.0);
        let out = leaky_relu(neg, 4);
        // Still negative, but closer to zero than the input.
        assert!(out.value < 0);
        assert!(out.value > neg.value);
    }

    #[test]
    fn leaky_relu_vec_applies_elementwise() {
        let xs = vec![FixedPoint::quantize(-1.0), FixedPoint::quantize(2.0)];
        let out = leaky_relu_vec(&xs, 4);
        assert_eq!(out.len(), 2);
        assert!(out[0].value < 0 && out[0].value > xs[0].value);
        assert_eq!(out[1].value, xs[1].value);
    }
}
