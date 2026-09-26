#!/usr/bin/env python3
"""
Compute expected values for golden test vectors.

This script helps verify the hand-computed Q16.16 fixed-point values
in the test vector JSON files. It uses the same arithmetic as the
Rust inference engine.

Q16.16 format: value = round(real_value * 2^16)
"""

SCALE = 16
SCALE_FACTOR = 1 << SCALE  # 65536


def quantize(real: float) -> int:
    """Convert a floating-point value to Q16.16 fixed-point."""
    return int(round(real * SCALE_FACTOR))


def dequantize(fixed: int) -> float:
    """Convert a Q16.16 fixed-point value to floating-point."""
    return fixed / SCALE_FACTOR


def logistic_regression_output(weights, inputs, bias):
    """
    Compute logistic regression output: dot(weights, inputs) + bias.
    
    Uses the same scaling as the Rust implementation:
    - Each product (w * x) is scaled by SCALE_FACTOR
    - Results are right-shifted by SCALE (divided by 2^16)
    - Sum is added to bias
    """
    dot = 0
    for w, x in zip(weights, inputs):
        # Product in Q16.16, then right-shift to maintain scale
        product = (w * x) >> SCALE
        dot += product
    return dot + bias


def main():
    """Verify the hand-computed values in the test vectors."""
    
    print("=== Logistic Regression Examples ===\n")
    
    # Positive score example
    print("Positive score (logistic_regression_positive.json):")
    weights = [quantize(1.0), quantize(2.0)]
    inputs = [quantize(1.0), quantize(2.0)]
    bias = quantize(0.5)
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  Weights: {[dequantize(w) for w in weights]}")
    print(f"  Inputs:  {[dequantize(x) for x in inputs]}")
    print(f"  Bias:    {dequantize(bias)}")
    print(f"  Expected: 1.0*1.0 + 2.0*2.0 + 0.5 = 5.5")
    print(f"  Computed raw: {result}")
    print(f"  Computed float: {dequantize(result)}")
    print(f"  Expected raw: {quantize(5.5)}")
    print(f"  Match: {result == quantize(5.5)}")
    print()
    
    # Negative weights with positive inputs
    print("Negative weights, positive inputs (logistic_regression_negative.json):")
    weights = [quantize(-1.0), quantize(-2.0)]
    inputs = [quantize(1.0), quantize(2.0)]
    bias = quantize(0.0)
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  Expected: -1.0*1.0 + -2.0*2.0 + 0 = -5.0")
    print(f"  Computed raw: {result}")
    print(f"  Computed float: {dequantize(result)}")
    print(f"  Expected raw: {quantize(-5.0)}")
    print(f"  Match: {result == quantize(-5.0)}")
    print()
    
    # Negative weights with negative inputs
    print("Negative weights, negative inputs (logistic_regression_negative.json):")
    weights = [quantize(-1.0), quantize(-2.0)]
    inputs = [quantize(-1.0), quantize(-2.0)]
    bias = quantize(0.0)
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  Expected: -1.0*-1.0 + -2.0*-2.0 + 0 = 5.0")
    print(f"  Computed raw: {result}")
    print(f"  Computed float: {dequantize(result)}")
    print(f"  Expected raw: {quantize(5.0)}")
    print(f"  Match: {result == quantize(5.0)}")
    print()
    
    # Zero score example
    print("Zero score (logistic_regression_zero.json):")
    weights = [quantize(1.0), quantize(-1.0)]
    inputs = [quantize(1.0), quantize(1.0)]
    bias = quantize(0.0)
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  Expected: 1.0*1.0 + -1.0*1.0 + 0 = 0.0")
    print(f"  Computed raw: {result}")
    print(f"  Computed float: {dequantize(result)}")
    print(f"  Expected raw: {quantize(0.0)}")
    print(f"  Match: {result == quantize(0.0)}")
    print()
    
    print("=== Logistic Regression Edge Cases ===\n")

    # Exactly at the threshold: score == decision_threshold, so class_label = 1.
    print("At the threshold (logistic_regression_at_threshold.json):")
    weights = [quantize(1.0), quantize(2.0)]
    inputs = [quantize(1.0), quantize(2.0)]
    bias = quantize(0.5)
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  1.0*1.0 + 2.0*2.0 + 0.5 = {dequantize(result)}")
    print(f"  raw: {result}  (threshold is the same value, so score >= threshold)")
    print()

    # A negative bias large enough to flip a positive dot product.
    print("Negative bias flips the decision (logistic_regression_negative_bias.json):")
    weights = [quantize(1.0), quantize(2.0)]
    inputs = [quantize(1.0), quantize(1.0)]
    bias = quantize(-4.25)
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  1.0*1.0 + 2.0*1.0 + -4.25 = {dequantize(result)}")
    print(f"  raw: {result}")
    print()

    # Mixed signs, exact case: every product lands on a whole Q16.16 step.
    print("Mixed signs, exact (logistic_regression_mixed_signs.json):")
    weights = [quantize(1.5), quantize(-0.75), quantize(2.25)]
    inputs = [quantize(-2.0), quantize(4.0), quantize(0.5)]
    bias = quantize(0.25)
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  1.5*-2.0 + -0.75*4.0 + 2.25*0.5 + 0.25 = {dequantize(result)}")
    print(f"  raw: {result}")
    print()

    # Mixed signs, inexact case: the shift floors, and it floors downwards on
    # negative products too, which is where a naive truncation would disagree.
    print("Mixed signs, inexact (logistic_regression_mixed_signs.json):")
    weights = [quantize(1.5), quantize(-0.75), quantize(2.25)]
    inputs = [quantize(0.1), quantize(-0.3), quantize(0.7)]
    bias = quantize(0.25)
    for w, x in zip(weights, inputs):
        product = (w * x) >> SCALE
        exact = dequantize(w) * dequantize(x)
        print(f"  {dequantize(w)} * {dequantize(x)}: raw {product} "
              f"({dequantize(product)}), exact {exact}")
    result = logistic_regression_output(weights, inputs, bias)
    print(f"  total raw: {result}  ({dequantize(result)})")
    print()

    print("=== Decision Tree Boundary Example ===\n")
    print("Threshold at 0.5 (Q16.16 = 32768):")
    print(f"  Input 0.0 (raw 0) -> 0 <= 32768 -> LEFT")
    print(f"  Input 0.5 (raw 32768) -> 32768 <= 32768 -> LEFT (<= semantics)")
    print(f"  Input 1.0 (raw 65536) -> 65536 > 32768 -> RIGHT")
    print()


if __name__ == "__main__":
    main()
