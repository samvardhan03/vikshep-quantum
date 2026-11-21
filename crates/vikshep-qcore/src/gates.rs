//! Gate matrices (`spec/VQS-1.md` section 6).
//!
//! Matrices are built on the host (Tier 2): every real component is computed
//! in binary64 by the expression the specification writes, with sine and
//! cosine from `vikshep-detmath`, rounded once to binary32 and flushed. The
//! resulting bytes are identical on every platform, and backends receive
//! them as data; they never recompute a matrix.
//!
//! A matrix of a gate on `k` targets has `2^k * 2^k` entries in row-major
//! order. Bit `m` of a row or column index is the value of target `m`
//! (the first target is the least significant bit).

use vikshep_detmath::{cos, sin};

use crate::arith::{C32, flush};

/// `fl32(1 / sqrt(2))`.
pub const INV_SQRT2: f32 = f32::from_bits(0x3F35_04F3);

/// A gate, without the qubits it acts on. Controlled gates are a gate plus
/// control qubits (see [`crate::circuit::Op`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Gate {
    /// Identity.
    I,
    /// Pauli X.
    X,
    /// Pauli Y.
    Y,
    /// Pauli Z.
    Z,
    /// Hadamard.
    H,
    /// Phase gate `diag(1, i)`.
    S,
    /// `diag(1, -i)`.
    Sdg,
    /// `diag(1, e^{i pi/4})`.
    T,
    /// `diag(1, e^{-i pi/4})`.
    Tdg,
    /// Square root of X.
    Sx,
    /// Rotation about X by the angle (radians).
    Rx(f64),
    /// Rotation about Y by the angle (radians).
    Ry(f64),
    /// Rotation about Z by the angle (radians).
    Rz(f64),
    /// Phase `diag(1, e^{i lambda})`.
    P(f64),
    /// General single-qubit gate `U(theta, phi, lambda)` (OpenQASM 3).
    U(f64, f64, f64),
    /// Swap of two qubits.
    Swap,
    /// An explicit matrix on `k` targets: `4^k` entries, row-major.
    Unitary {
        /// Number of target qubits.
        k: u32,
        /// The entries (flushed when the gate is built into an operation).
        matrix: Vec<C32>,
    },
}

/// Round a binary64 value once to binary32 and flush it (VQS-1 section 6.1).
#[inline]
fn r32(x: f64) -> f32 {
    flush(x as f32)
}

/// `(cos, sin)` of half the angle, in binary64.
#[inline]
fn half_angle(theta: f64) -> (f64, f64) {
    let h = theta * 0.5;
    (cos(h), sin(h))
}

const fn c(re: f32, im: f32) -> C32 {
    C32::new(re, im)
}

const O: C32 = C32::ZERO;
const ONE: C32 = C32::ONE;

impl Gate {
    /// Number of target qubits.
    #[must_use]
    pub fn num_targets(&self) -> u32 {
        match self {
            Self::Swap => 2,
            Self::Unitary { k, .. } => *k,
            _ => 1,
        }
    }

    /// Canonical lowercase name (VQS-1 section 10.1).
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::I => "id",
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::H => "h",
            Self::S => "s",
            Self::Sdg => "sdg",
            Self::T => "t",
            Self::Tdg => "tdg",
            Self::Sx => "sx",
            Self::Rx(_) => "rx",
            Self::Ry(_) => "ry",
            Self::Rz(_) => "rz",
            Self::P(_) => "p",
            Self::U(..) => "u",
            Self::Swap => "swap",
            Self::Unitary { .. } => "unitary",
        }
    }

    /// Parameters, in the order the gate takes them.
    #[must_use]
    pub fn params(&self) -> Vec<f64> {
        match self {
            Self::Rx(t) | Self::Ry(t) | Self::Rz(t) | Self::P(t) => vec![*t],
            Self::U(t, p, l) => vec![*t, *p, *l],
            _ => Vec::new(),
        }
    }

    /// The matrix, row-major, every component flushed.
    #[must_use]
    pub fn matrix(&self) -> Vec<C32> {
        let h = INV_SQRT2;
        match self {
            Self::I => vec![ONE, O, O, ONE],
            Self::X => vec![O, ONE, ONE, O],
            Self::Y => vec![O, c(0.0, -1.0), c(0.0, 1.0), O],
            Self::Z => vec![ONE, O, O, c(-1.0, 0.0)],
            Self::H => vec![c(h, 0.0), c(h, 0.0), c(h, 0.0), c(-h, 0.0)],
            Self::S => vec![ONE, O, O, c(0.0, 1.0)],
            Self::Sdg => vec![ONE, O, O, c(0.0, -1.0)],
            Self::T => vec![ONE, O, O, c(h, h)],
            Self::Tdg => vec![ONE, O, O, c(h, -h)],
            Self::Sx => vec![c(0.5, 0.5), c(0.5, -0.5), c(0.5, -0.5), c(0.5, 0.5)],
            Self::Rx(theta) => {
                let (ch, sh) = half_angle(*theta);
                let (cr, sr) = (r32(ch), r32(sh));
                vec![c(cr, 0.0), c(0.0, -sr), c(0.0, -sr), c(cr, 0.0)]
            }
            Self::Ry(theta) => {
                let (ch, sh) = half_angle(*theta);
                let (cr, sr) = (r32(ch), r32(sh));
                vec![c(cr, 0.0), c(-sr, 0.0), c(sr, 0.0), c(cr, 0.0)]
            }
            Self::Rz(theta) => {
                let (ch, sh) = half_angle(*theta);
                let (cr, sr) = (r32(ch), r32(sh));
                vec![c(cr, -sr), O, O, c(cr, sr)]
            }
            Self::P(lambda) => {
                vec![ONE, O, O, c(r32(cos(*lambda)), r32(sin(*lambda)))]
            }
            Self::U(theta, phi, lambda) => {
                let (ch, sh) = half_angle(*theta);
                let (cl, sl) = (cos(*lambda), sin(*lambda));
                let (cp, sp) = (cos(*phi), sin(*phi));
                let sum = *phi + *lambda;
                let (cs, ss) = (cos(sum), sin(sum));
                vec![
                    c(r32(ch), 0.0),
                    c(r32(-(cl * sh)), r32(-(sl * sh))),
                    c(r32(cp * sh), r32(sp * sh)),
                    c(r32(cs * ch), r32(ss * ch)),
                ]
            }
            Self::Swap => {
                let mut m = vec![O; 16];
                m[0] = ONE; // |00> -> |00>
                m[4 + 2] = ONE; // row 1, column 2
                m[2 * 4 + 1] = ONE; // row 2, column 1
                m[15] = ONE; // |11> -> |11>
                m
            }
            Self::Unitary { matrix, .. } => matrix.iter().map(|z| z.flushed()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(m: &[C32]) -> Vec<[u32; 2]> {
        m.iter().map(|z| z.to_bits()).collect()
    }

    #[test]
    fn inv_sqrt2_is_the_nearest_binary32() {
        assert_eq!(
            INV_SQRT2.to_bits(),
            std::f32::consts::FRAC_1_SQRT_2.to_bits()
        );
        assert_eq!(INV_SQRT2.to_bits(), (0.5f64.sqrt() as f32).to_bits());
    }

    #[test]
    fn zero_angle_rotations_carry_the_specified_signed_zeros() {
        // sin(+0) = +0, so the entry (+0, -s) is (+0, -0).
        let rx = Gate::Rx(0.0).matrix();
        assert_eq!(rx[0].to_bits(), [1.0f32.to_bits(), 0]);
        assert_eq!(rx[1].to_bits(), [0, 0x8000_0000]);
        let ry = Gate::Ry(0.0).matrix();
        assert_eq!(ry[1].to_bits(), [0x8000_0000, 0]);
        assert_eq!(ry[2].to_bits(), [0, 0]);
        let rz = Gate::Rz(0.0).matrix();
        assert_eq!(rz[0].to_bits(), [1.0f32.to_bits(), 0x8000_0000]);
        assert_eq!(rz[3].to_bits(), [1.0f32.to_bits(), 0]);
    }

    #[test]
    fn u_reduces_to_the_named_gates_up_to_rounding() {
        let close = |a: &[C32], b: &[C32]| {
            a.iter()
                .zip(b)
                .all(|(x, y)| (x.re - y.re).abs() < 1e-6 && (x.im - y.im).abs() < 1e-6)
        };
        let pi = std::f64::consts::PI;
        // U(pi/2, 0, pi) = H.
        assert!(close(
            &Gate::U(pi / 2.0, 0.0, pi).matrix(),
            &Gate::H.matrix()
        ));
        // U(pi, 0, pi) = X.
        assert!(close(&Gate::U(pi, 0.0, pi).matrix(), &Gate::X.matrix()));
        // U(theta, -pi/2, pi/2) = RX(theta).
        assert!(close(
            &Gate::U(0.7, -pi / 2.0, pi / 2.0).matrix(),
            &Gate::Rx(0.7).matrix()
        ));
        // U(theta, 0, 0) = RY(theta).
        assert!(close(
            &Gate::U(0.7, 0.0, 0.0).matrix(),
            &Gate::Ry(0.7).matrix()
        ));
        // U(0, 0, lambda) = P(lambda).
        assert!(close(
            &Gate::U(0.0, 0.0, 1.1).matrix(),
            &Gate::P(1.1).matrix()
        ));
    }

    #[test]
    fn unitary_entries_are_flushed() {
        let tiny = f32::from_bits(1);
        let g = Gate::Unitary {
            k: 1,
            matrix: vec![c(tiny, -tiny), O, O, ONE],
        };
        assert_eq!(bits(&g.matrix())[0], [0, 0x8000_0000]);
    }

    #[test]
    fn swap_matrix_exchanges_the_two_middle_basis_states() {
        let m = Gate::Swap.matrix();
        let ones: Vec<usize> = (0..16).filter(|&i| m[i] == ONE).collect();
        assert_eq!(ones, [0, 6, 9, 15]);
    }
}
