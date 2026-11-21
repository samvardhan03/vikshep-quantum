//! Tier-1 arithmetic (`spec/VQS-1.md` section 4).
//!
//! Every Tier-1 value is IEEE-754 binary32. The only operations are
//! addition, subtraction, multiplication, exact negation and comparison,
//! each rounded once (round-to-nearest, ties-to-even) and followed by the
//! VDS-1 flush: a result whose exponent field is zero becomes the zero of
//! the same sign, and a multiplication is flushed when its exact product is
//! below `FLT_MIN` (VDS-1 sections 8.1 and 8.3). The flush functions are the
//! Vikshep open core's, so this crate and the scattering engine share one
//! definition.
//!
//! Rust never contracts `a * b + c` into a fused multiply-add and never
//! reassociates, so the expressions below are evaluated exactly as written.

use vikshep_numerics::flush::{ftz, mul_tier1};

/// Tier-1 addition: `ftz(fl32(a + b))`.
#[inline(always)]
#[must_use]
pub fn add(a: f32, b: f32) -> f32 {
    ftz(a + b)
}

/// Tier-1 subtraction: `ftz(fl32(a - b))`.
#[inline(always)]
#[must_use]
pub fn sub(a: f32, b: f32) -> f32 {
    ftz(a - b)
}

/// Tier-1 multiplication with before-rounding tininess (VDS-1 section 8.3).
#[inline(always)]
#[must_use]
pub fn mul(a: f32, b: f32) -> f32 {
    mul_tier1(a, b)
}

/// A complex number with binary32 components.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C32 {
    /// Real part.
    pub re: f32,
    /// Imaginary part.
    pub im: f32,
}

impl C32 {
    /// `+0 + 0i`.
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    /// `1 + 0i`.
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };

    /// Build from components.
    #[inline]
    #[must_use]
    pub const fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    /// Both components flushed (host-side flushing of inputs and tables).
    #[inline]
    #[must_use]
    pub fn flushed(self) -> Self {
        Self {
            re: ftz(self.re),
            im: ftz(self.im),
        }
    }

    /// Both components are finite.
    #[inline]
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }

    /// Bit patterns of the two components.
    #[inline]
    #[must_use]
    pub fn to_bits(self) -> [u32; 2] {
        [self.re.to_bits(), self.im.to_bits()]
    }
}

/// Complex product (VQS-1 section 4.2):
///
/// ```text
/// re = sub(mul(a.re, b.re), mul(a.im, b.im))
/// im = add(mul(a.re, b.im), mul(a.im, b.re))
/// ```
#[inline(always)]
#[must_use]
pub fn cmul(a: C32, b: C32) -> C32 {
    C32 {
        re: sub(mul(a.re, b.re), mul(a.im, b.im)),
        im: add(mul(a.re, b.im), mul(a.im, b.re)),
    }
}

/// Complex sum, component by component (VQS-1 section 4.2).
#[inline(always)]
#[must_use]
pub fn cadd(a: C32, b: C32) -> C32 {
    C32 {
        re: add(a.re, b.re),
        im: add(a.im, b.im),
    }
}

/// Squared modulus `add(mul(re, re), mul(im, im))` (VQS-1 section 7.1).
#[inline(always)]
#[must_use]
pub fn norm_sq(a: C32) -> f32 {
    add(mul(a.re, a.re), mul(a.im, a.im))
}

/// Flush a binary32 value (re-exported for host-side input flushing).
#[inline(always)]
#[must_use]
pub fn flush(x: f32) -> f32 {
    ftz(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_product_follows_the_written_order() {
        let a = C32::new(1.5, -2.0);
        let b = C32::new(0.25, 4.0);
        let p = cmul(a, b);
        assert_eq!(p.re.to_bits(), (1.5f32 * 0.25 - (-2.0f32 * 4.0)).to_bits());
        assert_eq!(p.im.to_bits(), (1.5f32 * 4.0 + (-2.0f32 * 0.25)).to_bits());
    }

    #[test]
    fn subnormal_results_are_flushed_with_their_sign() {
        let min = f32::MIN_POSITIVE;
        // FLT_MIN - FLT_MIN * (1 + 2^-23) = -2^-149: subnormal, flushed to -0.
        let next = f32::from_bits(min.to_bits() + 1);
        assert_eq!(sub(min, next).to_bits(), 0x8000_0000);
        assert_eq!(sub(next, min).to_bits(), 0);
        // Exact product below FLT_MIN that rounds up to FLT_MIN: flushed.
        let almost_one = f32::from_bits(0x3f7f_ffff);
        assert_eq!((min * almost_one).to_bits(), min.to_bits());
        assert_eq!(mul(min, almost_one).to_bits(), 0);
        assert_eq!(mul(-min, almost_one).to_bits(), 0x8000_0000);
    }

    #[test]
    fn adding_a_zero_term_can_change_the_sign_of_zero() {
        // Why VQS-1 section 5.3 forbids skipping zero matrix entries unless
        // the result is proven bit-identical.
        assert_eq!(add(-0.0, 0.0).to_bits(), 0);
        assert_eq!(add(-0.0, -0.0).to_bits(), 0x8000_0000);
    }
}
