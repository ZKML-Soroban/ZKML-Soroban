//! BN254 point validation, in plain Rust.
//!
//! Soroban's BN254 host functions trap on a point they cannot parse. A trap
//! aborts the transaction, so a corrupted proof is still rejected, but the
//! caller learns nothing about why. Checking the points first turns that into
//! a typed error.
//!
//! G1 has a host function, `g1_is_on_curve`, that returns a `bool` instead of
//! trapping, but it still traps on a coordinate that is not reduced modulo `p`.
//! So G1 needs only the range check here. G2 has no host function at all, which
//! is why its on-curve check is implemented below.
//!
//! Multiplication is Montgomery's (CIOS). A first version used double-and-add,
//! which has no constants to get wrong, but measured on the compiled WASM it
//! added about 25 million instructions to `verify_receipt`, a 76% increase for
//! an honest proof. Montgomery needs a few constants instead; each is checked
//! in the tests below, and `crates/zkml-prover/tests/bn254_points.rs` compares
//! the curve checks with `ark-bn254` on thousands of points. A bug here would
//! reject valid proofs, so that comparison is the real safeguard.
//!
//! The curve checks never convert into Montgomery form. A Montgomery product
//! of plain values is `a * b * R^-1`, so both sides of `y^2 = x^3 + b` come out
//! scaled by `R^-2`, and `b` is precomputed with the same scale.
//!
//! Byte layouts follow Soroban (CAP-0074): G1 is `be(x) || be(y)`, G2 is
//! `be(x.c1) || be(x.c0) || be(y.c1) || be(y.c0)`, and all-zero bytes encode the
//! point at infinity.

/// A base field element as four little-endian 64-bit limbs.
type Fp = [u64; 4];

/// The base field modulus `p`.
const P: Fp = [
    0x3c20_8c16_d87c_fd47,
    0x9781_6a91_6871_ca8d,
    0xb850_45b6_8181_585d,
    0x3064_4e72_e131_a029,
];

/// `-p^-1 mod 2^64`, the Montgomery reduction constant.
const INV: u64 = 0x87d2_0782_e486_6389;

/// `3 * R^-2 mod p`, where `R = 2^256`: G1's `b`, at the scale the curve check
/// works in.
const B1_SCALED: Fp = [
    0xe22a_a9ce_bba9_97e1,
    0xefe1_e706_9e50_25a2,
    0x2fa2_a82e_82e9_2ead,
    0x2a84_6dad_38ab_a7a8,
];

/// `b' * R^-2 mod p`, where `b' = 3 / (9 + u)` is the G2 twist constant from
/// `ark_bn254::g2::Config::COEFF_B`.
const B2_SCALED: Fp2 = Fp2 {
    c0: [
        0xa722_0635_da39_3c03,
        0x928e_0ead_353d_8653,
        0x1ad4_d0e2_4534_4f6a,
        0x0b28_7a76_b729_1b66,
    ],
    c1: [
        0xc1e7_01ff_17dc_86f3,
        0x9f9b_327e_d867_c8c0,
        0x0988_6a0f_6a6c_43b3,
        0x0983_920c_5694_aec5,
    ],
};

/// One, for scaling a Montgomery product down once more.
const ONE: Fp = [1, 0, 0, 0];

#[cfg(test)]
const ZERO: Fp = [0; 4];

/// `a >= b`, comparing from the most significant limb.
fn geq(a: &Fp, b: &Fp) -> bool {
    for i in (0..4).rev() {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    true
}

/// `a - b`, assuming `a >= b`.
fn sub_raw(a: &Fp, b: &Fp) -> Fp {
    let mut out = [0u64; 4];
    let mut borrow = 0u64;
    for i in 0..4 {
        let (d1, b1) = a[i].overflowing_sub(b[i]);
        let (d2, b2) = d1.overflowing_sub(borrow);
        out[i] = d2;
        borrow = (b1 | b2) as u64;
    }
    out
}

/// `(a + b) mod p`, for `a, b < p`.
///
/// `p < 2^254`, so `a + b < 2^255` and never carries out of the top limb.
fn add_mod(a: &Fp, b: &Fp) -> Fp {
    let mut out = [0u64; 4];
    let mut carry = 0u64;
    for i in 0..4 {
        let (s1, c1) = a[i].overflowing_add(b[i]);
        let (s2, c2) = s1.overflowing_add(carry);
        out[i] = s2;
        carry = (c1 | c2) as u64;
    }
    if geq(&out, &P) {
        sub_raw(&out, &P)
    } else {
        out
    }
}

/// `(a - b) mod p`, for `a, b < p`.
fn sub_mod(a: &Fp, b: &Fp) -> Fp {
    if geq(a, b) {
        sub_raw(a, b)
    } else {
        // a - b + p, computed as p - (b - a) so nothing goes negative.
        sub_raw(&P, &sub_raw(b, a))
    }
}

/// `a * b * R^-1 mod p`, for `a, b < p`: Montgomery multiplication (CIOS).
///
/// `p < 2^254` leaves two spare bits, so the running total fits in four limbs
/// plus a carry that always ends at zero, and one conditional subtraction
/// finishes the reduction.
fn mont_mul(a: &Fp, b: &Fp) -> Fp {
    let mut t = [0u64; 6];
    for &b_i in b.iter() {
        // t += a * b_i
        let mut carry = 0u64;
        for j in 0..4 {
            let prod = (a[j] as u128) * (b_i as u128) + (t[j] as u128) + (carry as u128);
            t[j] = prod as u64;
            carry = (prod >> 64) as u64;
        }
        let sum = (t[4] as u128) + (carry as u128);
        t[4] = sum as u64;
        t[5] = (sum >> 64) as u64;

        // t = (t + m * p) / 2^64, with m chosen so the low limb cancels.
        let m = t[0].wrapping_mul(INV);
        let prod = (m as u128) * (P[0] as u128) + (t[0] as u128);
        let mut carry = (prod >> 64) as u64;
        for j in 1..4 {
            let prod = (m as u128) * (P[j] as u128) + (t[j] as u128) + (carry as u128);
            t[j - 1] = prod as u64;
            carry = (prod >> 64) as u64;
        }
        let sum = (t[4] as u128) + (carry as u128);
        t[3] = sum as u64;
        t[4] = t[5] + ((sum >> 64) as u64);
        t[5] = 0;
    }
    debug_assert_eq!(t[4], 0, "the spare bits of p keep this limb empty");
    let out = [t[0], t[1], t[2], t[3]];
    if geq(&out, &P) {
        sub_raw(&out, &P)
    } else {
        out
    }
}

/// `(a * b) mod p`, by double-and-add over the bits of `b`.
///
/// Too slow for the contract, and kept only as an independent reference for
/// testing `mont_mul`: it shares no constants with it.
#[cfg(test)]
fn mul_mod(a: &Fp, b: &Fp) -> Fp {
    let mut acc = ZERO;
    for limb in (0..4).rev() {
        for bit in (0..64).rev() {
            acc = add_mod(&acc, &acc);
            if (b[limb] >> bit) & 1 == 1 {
                acc = add_mod(&acc, a);
            }
        }
    }
    acc
}

/// Read 32 big-endian bytes as a field element, or `None` if they are not a
/// reduced value below `p`.
///
/// Anything `>= p` is rejected rather than reduced: Soroban's host rejects it
/// too, and accepting it here would let a check pass that the host then traps
/// on. This also covers the two flag bits Soroban requires to be clear, since
/// `p < 2^254`.
fn fp_from_be(bytes: &[u8]) -> Option<Fp> {
    let mut out = [0u64; 4];
    for (i, limb) in out.iter_mut().enumerate() {
        // limb 0 is the least significant, so it comes from the end.
        let start = 32 - 8 * (i + 1);
        let mut word = [0u8; 8];
        word.copy_from_slice(&bytes[start..start + 8]);
        *limb = u64::from_be_bytes(word);
    }
    if geq(&out, &P) {
        None
    } else {
        Some(out)
    }
}

/// An element of the quadratic extension `Fp[u] / (u^2 + 1)`: `c0 + c1 * u`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Fp2 {
    c0: Fp,
    c1: Fp,
}

fn fp2_add(a: &Fp2, b: &Fp2) -> Fp2 {
    Fp2 {
        c0: add_mod(&a.c0, &b.c0),
        c1: add_mod(&a.c1, &b.c1),
    }
}

/// `(a0 + a1 u)(b0 + b1 u) R^-1`, using `u^2 = -1`:
/// `(a0 b0 - a1 b1) + (a0 b1 + a1 b0) u`, each product scaled by `R^-1`.
fn fp2_mont_mul(a: &Fp2, b: &Fp2) -> Fp2 {
    let a0b0 = mont_mul(&a.c0, &b.c0);
    let a1b1 = mont_mul(&a.c1, &b.c1);
    let a0b1 = mont_mul(&a.c0, &b.c1);
    let a1b0 = mont_mul(&a.c1, &b.c0);
    Fp2 {
        c0: sub_mod(&a0b0, &a1b1),
        c1: add_mod(&a0b1, &a1b0),
    }
}

/// True if all 32-byte coordinates in `bytes` are reduced below `p`.
fn coords_reduced(bytes: &[u8]) -> bool {
    bytes.chunks(32).all(|c| fp_from_be(c).is_some())
}

/// True if a Soroban-encoded G1 point has every coordinate below `p`.
///
/// This is the only part of G1 validation that needs doing here: after it
/// passes, the host's `g1_is_on_curve` returns a `bool` instead of trapping,
/// and G1 on BN254 has cofactor 1, so on the curve means in the group.
pub fn g1_coords_reduced(bytes: &[u8; 64]) -> bool {
    coords_reduced(bytes)
}

/// True if a Soroban-encoded G1 point is on `y^2 = x^3 + 3`.
///
/// The contract uses the host function instead, which is cheaper; this exists
/// so the arithmetic that G2 relies on can be checked against a curve with a
/// host implementation to agree with.
pub fn g1_is_on_curve(bytes: &[u8; 64]) -> bool {
    if bytes.iter().all(|b| *b == 0) {
        return true; // the point at infinity
    }
    let (x, y) = match (fp_from_be(&bytes[..32]), fp_from_be(&bytes[32..])) {
        (Some(x), Some(y)) => (x, y),
        _ => return false,
    };
    // Both sides at scale R^-2: y^2 R^-1 scaled once more, and x^3 R^-2.
    let lhs = mont_mul(&mont_mul(&y, &y), &ONE);
    let rhs = add_mod(&mont_mul(&mont_mul(&x, &x), &x), &B1_SCALED);
    lhs == rhs
}

/// True if a Soroban-encoded G2 point is on `y^2 = x^3 + b'`.
///
/// This does not check subgroup membership. A point on the curve but outside
/// the prime-order subgroup still traps in the host's pairing check, so such a
/// proof is still rejected, just without a typed error. That case does not
/// arise from corruption: a random change to a valid point lands back on the
/// curve with probability about `1/p`. It takes a deliberately crafted point.
pub fn g2_is_on_curve(bytes: &[u8; 128]) -> bool {
    if bytes.iter().all(|b| *b == 0) {
        return true; // the point at infinity
    }
    // Soroban order: x.c1, x.c0, y.c1, y.c0.
    let parts = [
        fp_from_be(&bytes[0..32]),
        fp_from_be(&bytes[32..64]),
        fp_from_be(&bytes[64..96]),
        fp_from_be(&bytes[96..128]),
    ];
    let [Some(x_c1), Some(x_c0), Some(y_c1), Some(y_c0)] = parts else {
        return false;
    };
    let x = Fp2 { c0: x_c0, c1: x_c1 };
    let y = Fp2 { c0: y_c0, c1: y_c1 };
    // Both sides at scale R^-2, as for G1. Scaling y^2 down by one more R^-1
    // needs no Fp2 product: it is the same factor applied to each part.
    let y2 = fp2_mont_mul(&y, &y);
    let lhs = Fp2 {
        c0: mont_mul(&y2.c0, &ONE),
        c1: mont_mul(&y2.c1, &ONE),
    };
    let rhs = fp2_add(&fp2_mont_mul(&fp2_mont_mul(&x, &x), &x), &B2_SCALED);
    lhs == rhs
}

/// True if every coordinate of a Soroban-encoded G2 point is below `p`.
pub fn g2_coords_reduced(bytes: &[u8; 128]) -> bool {
    coords_reduced(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The G1 generator, `(1, 2)`.
    fn g1_generator() -> [u8; 64] {
        let mut out = [0u8; 64];
        out[31] = 1;
        out[63] = 2;
        out
    }

    /// The G2 generator in Soroban order, derived from `ark_bn254` and checked
    /// against the curve equation independently of this module.
    fn g2_generator() -> [u8; 128] {
        let hex = "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
                   1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
                   090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
                   12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa";
        let mut out = [0u8; 128];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
        }
        out
    }

    fn fp(n: u64) -> Fp {
        [n, 0, 0, 0]
    }

    /// `R mod p`, by doubling 1 256 times, so the test does not trust a
    /// hard-coded constant to check the hard-coded constants.
    fn r_mod_p() -> Fp {
        let mut r = fp(1);
        for _ in 0..256 {
            r = add_mod(&r, &r);
        }
        r
    }

    /// A fixed pseudo-random sequence of values below p, so failures reproduce.
    fn values(n: usize) -> impl Iterator<Item = Fp> {
        let mut state = 0x243f_6a88_85a3_08d3u64;
        (0..n).map(move |_| {
            let mut limb = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            // Clearing the top bits keeps the value below 2^252 < p.
            [limb(), limb(), limb(), limb() & 0x0fff_ffff_ffff_ffff]
        })
    }

    #[test]
    fn inv_is_minus_p_inverse_mod_two_to_the_64() {
        assert_eq!(P[0].wrapping_mul(INV), u64::MAX);
    }

    #[test]
    fn montgomery_agrees_with_plain_multiplication() {
        // mont(mont(a, b), R^2) = a b R^-1 R^2 R^-1 = a b.
        let r = r_mod_p();
        let r2 = mul_mod(&r, &r);
        let mut checked = 0;
        for (a, b) in values(400).zip(values(407).skip(7)) {
            assert_eq!(mont_mul(&mont_mul(&a, &b), &r2), mul_mod(&a, &b));
            checked += 1;
        }
        // Edge values, where carries are most likely to go wrong.
        let p_minus_one = sub_raw(&P, &fp(1));
        for (a, b) in [
            (p_minus_one, p_minus_one),
            (p_minus_one, fp(1)),
            (ZERO, p_minus_one),
        ] {
            assert_eq!(mont_mul(&mont_mul(&a, &b), &r2), mul_mod(&a, &b));
            checked += 1;
        }
        assert!(checked > 400);
    }

    #[test]
    fn the_scaled_constants_are_b_times_r_to_the_minus_two() {
        let r = r_mod_p();
        let r2 = mul_mod(&r, &r);
        // B * R^-2 * R^2 = B
        assert_eq!(mul_mod(&B1_SCALED, &r2), fp(3));
        let b2 = Fp2 {
            c0: mul_mod(&B2_SCALED.c0, &r2),
            c1: mul_mod(&B2_SCALED.c1, &r2),
        };
        // b' * (9 + u) = 3: (9 c0 - c1) + (c0 + 9 c1) u, with u^2 = -1.
        let nine = fp(9);
        let re = sub_mod(&mul_mod(&nine, &b2.c0), &b2.c1);
        let im = add_mod(&b2.c0, &mul_mod(&nine, &b2.c1));
        assert_eq!((re, im), (fp(3), ZERO), "b' must be 3 / (9 + u)");
    }

    #[test]
    fn small_arithmetic_is_right() {
        assert_eq!(add_mod(&fp(2), &fp(3)), fp(5));
        assert_eq!(sub_mod(&fp(5), &fp(3)), fp(2));
        assert_eq!(mul_mod(&fp(6), &fp(7)), fp(42));
    }

    #[test]
    fn subtraction_wraps_modulo_p() {
        // 0 - 1 = p - 1
        let p_minus_one = sub_raw(&P, &fp(1));
        assert_eq!(sub_mod(&ZERO, &fp(1)), p_minus_one);
        // and adding 1 back gives 0, not p
        assert_eq!(add_mod(&p_minus_one, &fp(1)), ZERO);
    }

    #[test]
    fn multiplication_reduces_modulo_p() {
        // (p - 1)^2 = 1 mod p
        let p_minus_one = sub_raw(&P, &fp(1));
        assert_eq!(mul_mod(&p_minus_one, &p_minus_one), fp(1));
    }

    #[test]
    fn p_itself_is_not_a_reduced_coordinate() {
        let mut bytes = [0u8; 32];
        for (i, limb) in P.iter().enumerate() {
            let start = 32 - 8 * (i + 1);
            bytes[start..start + 8].copy_from_slice(&limb.to_be_bytes());
        }
        assert!(fp_from_be(&bytes).is_none(), "p must be rejected");
        bytes[31] -= 1;
        assert!(fp_from_be(&bytes).is_some(), "p - 1 must be accepted");
    }

    #[test]
    fn the_generators_are_on_their_curves() {
        assert!(g1_is_on_curve(&g1_generator()));
        assert!(g2_is_on_curve(&g2_generator()));
    }

    #[test]
    fn infinity_counts_as_on_the_curve() {
        // Soroban's encoding for the identity; the host accepts it.
        assert!(g1_is_on_curve(&[0u8; 64]));
        assert!(g2_is_on_curve(&[0u8; 128]));
    }

    #[test]
    fn every_single_byte_flip_of_the_g2_generator_is_caught() {
        // The property the contract relies on: no single-byte change to a
        // valid point survives as a valid point.
        let g = g2_generator();
        for i in 0..128 {
            let mut bad = g;
            bad[i] ^= 0x01;
            assert!(!g2_is_on_curve(&bad), "flipping byte {i} was not caught");
        }
    }

    #[test]
    fn every_single_byte_flip_of_the_g1_generator_is_caught() {
        let g = g1_generator();
        for i in 0..64 {
            let mut bad = g;
            bad[i] ^= 0x01;
            assert!(!g1_is_on_curve(&bad), "flipping byte {i} was not caught");
        }
    }

    #[test]
    fn a_coordinate_above_p_is_rejected_not_reduced() {
        let mut bad = g2_generator();
        bad[0] = 0xff; // x.c1 far above p, and the flag bits set
        assert!(!g2_coords_reduced(&bad));
        assert!(!g2_is_on_curve(&bad));
        let mut bad1 = g1_generator();
        bad1[0] = 0xff;
        assert!(!g1_coords_reduced(&bad1));
    }
}
