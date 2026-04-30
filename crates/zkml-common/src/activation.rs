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
