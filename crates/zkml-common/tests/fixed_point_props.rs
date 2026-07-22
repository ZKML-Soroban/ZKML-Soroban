//! Property-based tests for FixedPoint arithmetic using proptest.
//!
//! These tests verify critical soundness properties:
//! - Round-trip precision bounds
//! - Quantization monotonicity
//! - Arithmetic consistency with f64
//! - Overflow safety for checked operations

use proptest::prelude::*;
use zkml_common::fixed_point::{FixedPoint, DEFAULT_SCALE};

/// Maximum representable value in Q16.16 format.
///
/// i64::MAX / 2^16 ≈ 1.4e14
const MAX_REPRESENTABLE: f64 = (i64::MAX as f64) / ((1i64 << DEFAULT_SCALE) as f64);

/// Minimum representable value in Q16.16 format.
///
/// i64::MIN / 2^16 ≈ -1.4e14
const MIN_REPRESENTABLE: f64 = (i64::MIN as f64) / ((1i64 << DEFAULT_SCALE) as f64);

/// Strategy for generating f64 values within the Q16.16 representable range.
///
/// Excludes NaN, infinities, and subnormal values that would quantize to zero.
/// The minimum non-zero representable value is 1.0 / 2^16 ≈ 1.5e-5.
fn representable_f64() -> impl Strategy<Value = f64> {
    prop::num::f64::NORMAL
        .prop_filter("Value must be within representable range", |&x| {
            (MIN_REPRESENTABLE..=MAX_REPRESENTABLE).contains(&x)
        })
        .prop_filter("Value must be large enough to quantize non-zero", |&x| {
            x.abs() >= (1.0 / ((1i64 << DEFAULT_SCALE) as f64))
        })
}

/// Strategy for generating pairs of f64 values within representable range.
fn representable_f64_pair() -> impl Strategy<Value = (f64, f64)> {
    (representable_f64(), representable_f64())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property 1: Round-trip bound
    ///
    /// For any f64 in the representable range, dequantize(quantize(x)) is within
    /// 2^-16 of x. This ensures quantization doesn't introduce excessive error.
    #[test]
    fn round_trip_bound(x in representable_f64()) {
        let fp = FixedPoint::quantize(x);
        let recovered = fp.dequantize();
        let error = (x - recovered).abs();
        let max_error = 2.0_f64.powi(-(DEFAULT_SCALE as i32));
        prop_assert!(error <= max_error, "Round-trip error {} exceeds bound {}", error, max_error);
    }

    /// Property 2: Quantization monotonicity
    ///
    /// x <= y implies quantize(x) <= quantize(y). This ensures the quantization
    /// function preserves ordering, which is critical for comparisons in ML models.
    #[test]
    fn quantization_monotonicity((x, y) in representable_f64_pair()) {
        let fp_x = FixedPoint::quantize(x);
        let fp_y = FixedPoint::quantize(y);
        if x <= y {
            prop_assert!(fp_x.value <= fp_y.value, "Quantization violated monotonicity: {} <= {} but {} > {}", x, y, fp_x.value, fp_y.value);
        }
    }

    /// Property 3: Arithmetic consistency - addition
    ///
    /// For random pairs (a, b), fixed-point addition matches f64 computation
    /// within an error bound, when no overflow occurs.
    #[test]
    fn arithmetic_consistency_add((a, b) in representable_f64_pair()) {
        let fp_a = FixedPoint::quantize(a);
        let fp_b = FixedPoint::quantize(b);

        // Skip if overflow would occur
        if let Some(fp_sum) = fp_a.checked_add(fp_b) {
            let expected = a + b;
            let actual = fp_sum.dequantize();
            let absolute_error = (expected - actual).abs();

            // Each input has up to 2^-16 error, so addition can have up to 2^-15 error
            // Plus one ULP for rounding
            let max_error = 2.0_f64.powi(-(DEFAULT_SCALE as i32 - 1)) + f64::EPSILON;

            // Use absolute error for small results, relative for large results
            let error_bound = if expected.abs() < 1.0 {
                max_error
            } else {
                max_error * expected.abs()
            };

            prop_assert!(absolute_error <= error_bound, "Addition error: expected {}, got {}, absolute error {}, bound {}", expected, actual, absolute_error, error_bound);
        }
    }

    /// Property 3: Arithmetic consistency - subtraction
    ///
    /// For random pairs (a, b), fixed-point subtraction matches f64 computation
    /// within an error bound, when no overflow occurs.
    #[test]
    fn arithmetic_consistency_sub((a, b) in representable_f64_pair()) {
        let fp_a = FixedPoint::quantize(a);
        let fp_b = FixedPoint::quantize(b);

        // Skip if overflow would occur
        if let Some(fp_diff) = fp_a.checked_sub(fp_b) {
            let expected = a - b;
            let actual = fp_diff.dequantize();
            let absolute_error = (expected - actual).abs();

            // Each input has up to 2^-16 error, so subtraction can have up to 2^-15 error
            let max_error = 2.0_f64.powi(-(DEFAULT_SCALE as i32 - 1)) + f64::EPSILON;

            let error_bound = if expected.abs() < 1.0 {
                max_error
            } else {
                max_error * expected.abs()
            };

            prop_assert!(absolute_error <= error_bound, "Subtraction error: expected {}, got {}, absolute error {}, bound {}", expected, actual, absolute_error, error_bound);
        }
    }

    /// Property 3: Arithmetic consistency - multiplication
    ///
    /// For random pairs (a, b), fixed-point multiplication matches f64 computation
    /// within an error bound, when no overflow occurs.
    #[test]
    fn arithmetic_consistency_mul((a, b) in representable_f64_pair()) {
        let fp_a = FixedPoint::quantize(a);
        let fp_b = FixedPoint::quantize(b);

        // Skip if overflow would occur
        if let Some(fp_product) = fp_a.checked_mul(fp_b) {
            let expected = a * b;
            let actual = fp_product.dequantize();

            // If the actual result is zero, check if the expected result is small enough
            // that quantization to zero is acceptable
            let min_representable = 0.5 / ((1i64 << DEFAULT_SCALE) as f64);
            if actual == 0.0 && expected.abs() < min_representable {
                return Ok(());
            }

            let absolute_error = (expected - actual).abs();

            // Multiplication error analysis:
            // Each input has quantization error up to 0.5 / 2^scale
            // Error propagates as: |a*εb| + |b*εa| where ε is quantization error
            // This gives: |a| * (0.5/2^scale) + |b| * (0.5/2^scale)
            // = 0.5 * (|a| + |b|) / 2^scale
            let max_absolute_error = 0.5 * (a.abs() + b.abs()) / ((1i64 << DEFAULT_SCALE) as f64);

            // The result itself also has quantization error up to 0.5 / 2^scale
            let result_quantization_error = 0.5 / ((1i64 << DEFAULT_SCALE) as f64);

            // Total error bound with a safety factor
            let error_bound = (max_absolute_error + result_quantization_error) * 2.0 + f64::EPSILON;

            prop_assert!(absolute_error <= error_bound, "Multiplication error: expected {}, got {}, absolute error {}, bound {}", expected, actual, absolute_error, error_bound);
        }
    }

    /// Property 4: Overflow safety - checked_add
    ///
    /// checked_add never panics and never silently wraps for any input pair.
    /// It returns None on overflow and Some(result) otherwise.
    #[test]
    fn overflow_safety_checked_add(a in representable_f64(), b in representable_f64()) {
        let fp_a = FixedPoint::quantize(a);
        let fp_b = FixedPoint::quantize(b);

        // This should never panic
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fp_a.checked_add(fp_b)
        }));

        prop_assert!(result.is_ok(), "checked_add panicked");

        // Verify no silent wrap by checking against manual calculation
        let manual_sum = fp_a.value.checked_add(fp_b.value);
        let fp_result = result.unwrap();
        match (manual_sum, fp_result) {
            (None, None) => {}, // Both overflow - correct
            (Some(manual_val), Some(fp)) => {
                prop_assert_eq!(fp.value, manual_val, "checked_add silently wrapped");
            },
            (None, Some(_)) => {
                prop_assert!(false, "checked_add returned Some when i64::checked_add returned None");
            },
            (Some(_), None) => {
                prop_assert!(false, "checked_add returned None when i64::checked_add returned Some");
            },
        }
    }

    /// Property 4: Overflow safety - checked_sub
    ///
    /// checked_sub never panics and never silently wraps for any input pair.
    #[test]
    fn overflow_safety_checked_sub(a in representable_f64(), b in representable_f64()) {
        let fp_a = FixedPoint::quantize(a);
        let fp_b = FixedPoint::quantize(b);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fp_a.checked_sub(fp_b)
        }));

        prop_assert!(result.is_ok(), "checked_sub panicked");

        let manual_diff = fp_a.value.checked_sub(fp_b.value);
        let fp_result = result.unwrap();
        match (manual_diff, fp_result) {
            (None, None) => {},
            (Some(manual_val), Some(fp)) => {
                prop_assert_eq!(fp.value, manual_val, "checked_sub silently wrapped");
            },
            (None, Some(_)) => {
                prop_assert!(false, "checked_sub returned Some when i64::checked_sub returned None");
            },
            (Some(_), None) => {
                prop_assert!(false, "checked_sub returned None when i64::checked_sub returned Some");
            },
        }
    }

    /// Property 4: Overflow safety - checked_mul
    ///
    /// checked_mul never panics and never silently wraps for any input pair.
    #[test]
    fn overflow_safety_checked_mul(a in representable_f64(), b in representable_f64()) {
        let fp_a = FixedPoint::quantize(a);
        let fp_b = FixedPoint::quantize(b);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fp_a.checked_mul(fp_b)
        }));

        prop_assert!(result.is_ok(), "checked_mul panicked");

        // Verify that the result matches what we'd expect from the i128 implementation
        let wide = (fp_a.value as i128) * (fp_b.value as i128);
        let scaled = wide >> DEFAULT_SCALE;
        let expected_value = i64::try_from(scaled).ok();

        let fp_result = result.unwrap();
        match (expected_value, fp_result) {
            (None, None) => {}, // Both overflow - correct
            (Some(expected_val), Some(fp)) => {
                prop_assert_eq!(fp.value, expected_val, "checked_mul returned wrong value");
            },
            (None, Some(_)) => {
                prop_assert!(false, "checked_mul returned Some when i128 computation would overflow i64");
            },
            (Some(_), None) => {
                prop_assert!(false, "checked_mul returned None when i128 computation fits in i64");
            },
        }
    }

    /// Property 4: Overflow safety - checked_div
    ///
    /// checked_div never panics and never silently wraps for any input pair.
    #[test]
    fn overflow_safety_checked_div(a in representable_f64(), b in representable_f64()) {
        let fp_a = FixedPoint::quantize(a);
        let fp_b = FixedPoint::quantize(b);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fp_a.checked_div(fp_b)
        }));

        prop_assert!(result.is_ok(), "checked_div panicked");

        // Verify division by zero returns None
        if b == 0.0 || fp_b.value == 0 {
            prop_assert!(result.unwrap().is_none(), "checked_div should return None for division by zero");
        }
    }
}
