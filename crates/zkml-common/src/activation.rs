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
}
