//! The normative VQS-1 kernels, written over a host slice
//! (`spec/VQS-1.md` sections 3, 5, 7, 8 and 9).
//!
//! A state of `n` qubits is a slice of `2 * 2^n` binary32 values: amplitude
//! `i` is `(state[2 i], state[2 i + 1])`, and bit `q` of `i` is the value of
//! qubit `q`. Every function here is single-threaded and evaluates exactly
//! the expressions the specification writes; a backend conforms when its
//! results have the same bytes.
//!
//! The functions assume their arguments were validated (see
//! [`crate::circuit::Op::validate`] and [`check_state`]).

use vikshep_numerics::flush::ftz_slice;
use vikshep_numerics::rng::Stream;

use crate::arith::{C32, add, cadd, cmul, mul, norm_sq, sub};
use crate::circuit::validate_n_qubits;
use crate::error::QError;
use crate::pauli::PauliString;

/// Number of qubits of a state slice, if its length is `2 * 2^n`, `n >= 1`.
#[must_use]
pub fn qubits_of(len: usize) -> Option<u32> {
    if len < 4 || !len.is_power_of_two() {
        return None;
    }
    Some(len.trailing_zeros() - 1)
}

/// Check that `state` holds `n_qubits` qubits of finite values.
pub fn check_state(n_qubits: u32, state: &[f32]) -> Result<(), QError> {
    validate_n_qubits(n_qubits)?;
    let expected = state_len(n_qubits)?;
    if state.len() != expected {
        return Err(QError::StateSize {
            expected,
            got: state.len(),
        });
    }
    if state.iter().any(|v| !v.is_finite()) {
        return Err(QError::NonFinite("amplitude"));
    }
    Ok(())
}

/// Number of binary32 elements of an `n_qubits` state.
pub fn state_len(n_qubits: u32) -> Result<usize, QError> {
    1usize
        .checked_shl(n_qubits + 1)
        .filter(|&len| len != 0 && len.trailing_zeros() == n_qubits + 1)
        .ok_or(QError::OutOfMemory(n_qubits))
}

/// The state `|0...0>`: amplitude 0 is `(1, +0)`, every other `(+0, +0)`
/// (VQS-1 section 3.3).
pub fn zero_state(n_qubits: u32) -> Result<Vec<f32>, QError> {
    validate_n_qubits(n_qubits)?;
    let len = state_len(n_qubits)?;
    let mut state = Vec::new();
    state
        .try_reserve_exact(len)
        .map_err(|_| QError::OutOfMemory(n_qubits))?;
    state.resize(len, 0.0);
    state[0] = 1.0;
    Ok(state)
}

/// Flush every component of a state supplied by the caller (VQS-1 section
/// 3.4). The state is not normalized.
pub fn flush_state(state: &mut [f32]) {
    ftz_slice(state);
}

#[inline(always)]
fn amp(state: &[f32], i: usize) -> C32 {
    C32::new(state[2 * i], state[2 * i + 1])
}

/// Apply a matrix to `targets` where all `controls` are 1 (VQS-1 section 5).
///
/// For every index `b` whose target bits are 0 and whose control bits are 1:
///
/// ```text
/// x[c] = state[b | spread(c)]                         c = 0 .. 2^k - 1
/// y[r] = (M[r][0] * x[0]) + (M[r][1] * x[1]) + ...    left to right
/// state[b | spread(r)] = y[r]
/// ```
///
/// where `spread(c)` places bit `m` of `c` at qubit `targets[m]`, `*` is
/// [`cmul`] and `+` is [`cadd`]. No term is skipped, including terms whose
/// matrix entry is zero. Amplitudes outside the addressed groups keep their
/// bits.
pub fn apply_matrix(state: &mut [f32], matrix: &[C32], targets: &[u32], controls: &[u32]) {
    if targets.len() == 1 && controls.is_empty() {
        apply_one_target(state, matrix, targets[0]);
    } else {
        apply_general(state, matrix, targets, controls);
    }
}

/// The general kernel: any number of targets and controls.
pub fn apply_general(state: &mut [f32], matrix: &[C32], targets: &[u32], controls: &[u32]) {
    let k = targets.len();
    let dim = 1usize << k;
    debug_assert_eq!(matrix.len(), dim * dim);

    // Qubits fixed inside a group, ascending, for zero-bit insertion.
    let mut fixed: Vec<u32> = targets.iter().chain(controls).copied().collect();
    fixed.sort_unstable();
    let control_mask = controls.iter().fold(0usize, |m, &c| m | (1usize << c));
    let offsets: Vec<usize> = (0..dim)
        .map(|c| {
            targets
                .iter()
                .enumerate()
                .fold(0usize, |o, (m, &t)| o | (((c >> m) & 1) << t))
        })
        .collect();

    let groups = (state.len() / 2) >> fixed.len();
    let mut x = vec![C32::ZERO; dim];
    for g in 0..groups {
        // Insert a zero bit at every fixed position, lowest first.
        let mut base = g;
        for &p in &fixed {
            let low = base & ((1usize << p) - 1);
            base = ((base >> p) << (p + 1)) | low;
        }
        base |= control_mask;

        for (slot, &offset) in x.iter_mut().zip(&offsets) {
            *slot = amp(state, base | offset);
        }
        for (row, &offset) in matrix.chunks_exact(dim).zip(&offsets) {
            let mut acc = cmul(row[0], x[0]);
            for (&m, &xc) in row[1..].iter().zip(&x[1..]) {
                acc = cadd(acc, cmul(m, xc));
            }
            let i = 2 * (base | offset);
            state[i] = acc.re;
            state[i + 1] = acc.im;
        }
    }
}

/// One target, no controls: the same expressions as [`apply_general`] in a
/// loop without index arithmetic.
fn apply_one_target(state: &mut [f32], matrix: &[C32], target: u32) {
    debug_assert_eq!(matrix.len(), 4);
    let (m00, m01, m10, m11) = (matrix[0], matrix[1], matrix[2], matrix[3]);
    let half = 2usize << target; // binary32 elements between pair partners
    for block in state.chunks_exact_mut(2 * half) {
        let (lo, hi) = block.split_at_mut(half);
        for (a, b) in lo.chunks_exact_mut(2).zip(hi.chunks_exact_mut(2)) {
            let x0 = C32::new(a[0], a[1]);
            let x1 = C32::new(b[0], b[1]);
            let y0 = cadd(cmul(m00, x0), cmul(m01, x1));
            let y1 = cadd(cmul(m10, x0), cmul(m11, x1));
            a[0] = y0.re;
            a[1] = y0.im;
            b[0] = y1.re;
            b[1] = y1.im;
        }
    }
}

/// Probability leaf `i`: `add(mul(re, re), mul(im, im))` (VQS-1 section 7.1).
#[inline]
#[must_use]
pub fn probability(state: &[f32], i: usize) -> f32 {
    norm_sq(amp(state, i))
}

/// All probability leaves, in index order.
#[must_use]
pub fn probabilities(state: &[f32]) -> Vec<f32> {
    (0..state.len() / 2)
        .map(|i| probability(state, i))
        .collect()
}

/// Root of the pairwise tree over `2^n` leaves (VQS-1 section 7.2): leaves
/// `2 j` and `2 j + 1` are added first, then their sums, level by level,
/// left operand first. Uses a stack of at most `n + 1` partial sums.
#[must_use]
pub fn tree_root(n: u32, leaf: impl Fn(usize) -> f32) -> f32 {
    let mut stack = [0.0f32; 64];
    let mut depth = 0usize;
    for i in 0..(1usize << n) {
        let mut v = leaf(i);
        let mut k = i;
        while k & 1 == 1 {
            depth -= 1;
            v = add(stack[depth], v);
            k >>= 1;
        }
        stack[depth] = v;
        depth += 1;
    }
    stack[0]
}

/// Squared norm: the root of the probability tree.
#[must_use]
pub fn norm_sq_of(state: &[f32]) -> f32 {
    let n = qubits_of(state.len()).expect("validated state length");
    tree_root(n, |i| probability(state, i))
}

/// The pairwise tree of VQS-1 section 7.2, levels 1 to `n` (level 0, the
/// leaves, is recomputed from the state when needed).
#[derive(Clone, Debug, PartialEq)]
pub struct SumTree {
    n: u32,
    /// Level 1 (`2^(n-1)` nodes), then level 2, ..., then level `n` (root).
    nodes: Vec<f32>,
}

impl SumTree {
    /// Build the tree over `2^n` leaves, `n >= 1`.
    pub fn build(n: u32, leaf: impl Fn(usize) -> f32) -> Result<Self, QError> {
        let total = (1usize << n) - 1;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(total)
            .map_err(|_| QError::OutOfMemory(n))?;
        for j in 0..(1usize << (n - 1)) {
            nodes.push(add(leaf(2 * j), leaf(2 * j + 1)));
        }
        let mut below = 0usize; // start of the level below the one being built
        for level in 2..=n {
            let count = 1usize << (n - level);
            for j in 0..count {
                let v = add(nodes[below + 2 * j], nodes[below + 2 * j + 1]);
                nodes.push(v);
            }
            below += 2 * count;
        }
        Ok(Self { n, nodes })
    }

    /// The probability tree of a state.
    pub fn of_state(state: &[f32]) -> Result<Self, QError> {
        let n = qubits_of(state.len()).ok_or(QError::StateSize {
            expected: 4,
            got: state.len(),
        })?;
        Self::build(n, |i| probability(state, i))
    }

    /// Number of qubits (levels above the leaves).
    #[must_use]
    pub fn n_qubits(&self) -> u32 {
        self.n
    }

    fn offset(&self, level: u32) -> usize {
        (1usize << self.n) - (1usize << (self.n - level + 1))
    }

    /// Node `j` of `level` (1 to `n`).
    #[inline]
    #[must_use]
    pub fn node(&self, level: u32, j: usize) -> f32 {
        self.nodes[self.offset(level) + j]
    }

    /// All nodes of `level` (1 to `n`).
    #[must_use]
    pub fn level(&self, level: u32) -> &[f32] {
        let start = self.offset(level);
        &self.nodes[start..start + (1usize << (self.n - level))]
    }

    /// The root (level `n`).
    #[must_use]
    pub fn root(&self) -> f32 {
        self.nodes[self.nodes.len() - 1]
    }

    /// Levels 1 to `n`, concatenated.
    #[must_use]
    pub fn nodes(&self) -> &[f32] {
        &self.nodes
    }
}

/// Draw `shots` basis states (VQS-1 section 8).
///
/// Shot `s` starts at the root and descends one level per qubit, from qubit
/// `n - 1` to qubit 0. At a node with mass `m` and children `l` (bit 0) and
/// `r` (bit 1):
///
/// ```text
/// u   = (word(s * n + step) >> 8) * 2^-24      step = 0 .. n - 1
/// t   = mul(u, m)
/// bit = 0 if l > 0 and (t < l or r == 0), else 1
/// ```
///
/// `word` is the Philox stream `(master_seed, stream_id)` of VDS-1 section 5.
/// A child with zero mass is never entered, so every outcome has non-zero
/// probability.
pub fn sample(
    state: &[f32],
    tree: &SumTree,
    master_seed: u64,
    stream_id: u64,
    shots: u64,
) -> Result<Vec<u64>, QError> {
    let n = tree.n_qubits();
    let root = tree.root();
    if !(root > 0.0 && root.is_finite()) {
        return Err(QError::ZeroNorm);
    }
    let count = usize::try_from(shots).map_err(|_| QError::Backend("too many shots".into()))?;
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| QError::Backend("not enough memory for the requested shots".into()))?;
    let mut stream = Stream::new(master_seed, stream_id);
    for _ in 0..count {
        let mut j = 0usize;
        for level in (1..=n).rev() {
            let m = tree.node(level, j);
            let (l, r) = if level == 1 {
                (probability(state, 2 * j), probability(state, 2 * j + 1))
            } else {
                (tree.node(level - 1, 2 * j), tree.node(level - 1, 2 * j + 1))
            };
            let t = mul(stream.next_f32_unit(), m);
            let left = l > 0.0 && (t < l || r == 0.0);
            j = 2 * j + usize::from(!left);
        }
        out.push(j as u64);
    }
    Ok(out)
}

/// Expectation value of a Pauli string (VQS-1 section 9): the root of the
/// pairwise tree over the terms
///
/// ```text
/// j    = i XOR x_mask                  a = amplitude i, b = amplitude j
/// re   = add(mul(a.re, b.re), mul(a.im, b.im))     # Re(conj(a) b)
/// im   = sub(mul(a.re, b.im), mul(a.im, b.re))     # Im(conj(a) b)
/// v    = re, -im, -re, im   for (number of Y) mod 4 = 0, 1, 2, 3
/// term = -v if popcount(j AND z_mask) is odd, else v
/// ```
#[must_use]
pub fn expectation_pauli(state: &[f32], pauli: &PauliString) -> f32 {
    let n = qubits_of(state.len()).expect("validated state length");
    let x_mask = pauli.x_mask() as usize;
    let z_mask = pauli.z_mask() as usize;
    let y_phase = pauli.y_count() & 3;
    tree_root(n, |i| {
        let j = i ^ x_mask;
        let a = amp(state, i);
        let b = amp(state, j);
        let v = match y_phase {
            0 => add(mul(a.re, b.re), mul(a.im, b.im)),
            1 => -sub(mul(a.re, b.im), mul(a.im, b.re)),
            2 => -add(mul(a.re, b.re), mul(a.im, b.im)),
            _ => sub(mul(a.re, b.im), mul(a.im, b.re)),
        };
        if (j & z_mask).count_ones() & 1 == 1 {
            -v
        } else {
            v
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::Gate;

    fn test_state(n: u32, seed: u64) -> Vec<f32> {
        let mut s = Stream::new(seed, 0);
        (0..(2usize << n))
            .map(|_| s.next_f32_unit() * 2.0 - 1.0)
            .collect()
    }

    fn bits(v: &[f32]) -> Vec<u32> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    #[test]
    fn qubits_of_accepts_only_state_lengths() {
        assert_eq!(qubits_of(4), Some(1));
        assert_eq!(qubits_of(16), Some(3));
        assert_eq!(qubits_of(2), None);
        assert_eq!(qubits_of(12), None);
        assert_eq!(qubits_of(0), None);
    }

    #[test]
    fn one_target_path_matches_the_general_kernel_bit_for_bit() {
        for gate in [Gate::H, Gate::Y, Gate::Sx, Gate::U(0.3, 1.1, -2.0)] {
            let m = gate.matrix();
            for t in 0..5 {
                let mut a = test_state(5, 7);
                let mut b = a.clone();
                apply_matrix(&mut a, &m, &[t], &[]);
                apply_general(&mut b, &m, &[t], &[]);
                assert_eq!(bits(&a), bits(&b), "{gate:?} on qubit {t}");
            }
        }
    }

    #[test]
    fn bell_state_amplitudes() {
        let mut s = zero_state(2).unwrap();
        apply_matrix(&mut s, &Gate::H.matrix(), &[0], &[]);
        apply_matrix(&mut s, &Gate::X.matrix(), &[1], &[0]);
        let h = crate::gates::INV_SQRT2;
        assert_eq!(s, [h, 0.0, 0.0, 0.0, 0.0, 0.0, h, 0.0]);
    }

    #[test]
    fn controls_leave_other_amplitudes_untouched() {
        let before = test_state(4, 3);
        let mut after = before.clone();
        apply_matrix(&mut after, &Gate::H.matrix(), &[2], &[0, 3]);
        for i in 0..16usize {
            let addressed = i & 0b1001 == 0b1001;
            let same = bits(&before[2 * i..2 * i + 2]) == bits(&after[2 * i..2 * i + 2]);
            assert!(addressed || same, "amplitude {i} changed");
        }
    }

    #[test]
    fn target_order_selects_the_matrix_index_bits() {
        // X on the first listed target only: a 4x4 matrix X (x) I in the
        // "first target is the low bit" convention flips targets[0].
        let x_on_low = [
            C32::ZERO,
            C32::ONE,
            C32::ZERO,
            C32::ZERO,
            C32::ONE,
            C32::ZERO,
            C32::ZERO,
            C32::ZERO,
            C32::ZERO,
            C32::ZERO,
            C32::ZERO,
            C32::ONE,
            C32::ZERO,
            C32::ZERO,
            C32::ONE,
            C32::ZERO,
        ];
        let mut s = zero_state(3).unwrap();
        apply_matrix(&mut s, &x_on_low, &[2, 0], &[]);
        assert_eq!(s[2 * 0b100], 1.0); // qubit 2 flipped
        let mut s = zero_state(3).unwrap();
        apply_matrix(&mut s, &x_on_low, &[0, 2], &[]);
        assert_eq!(s[2], 1.0); // amplitude 0b001: qubit 0 flipped
    }

    #[test]
    fn stack_reduction_equals_the_stored_tree() {
        for n in 1..=7 {
            let s = test_state(n, 11 + u64::from(n));
            let tree = SumTree::of_state(&s).unwrap();
            assert_eq!(tree.root().to_bits(), norm_sq_of(&s).to_bits());
            assert_eq!(tree.nodes().len(), (1usize << n) - 1);
            assert_eq!(tree.level(n).len(), 1);
        }
        // Order: ((p0 + p1) + (p2 + p3)).
        let leaves = [1.0e8f32, 1.0, -1.0e8, 1.0];
        let root = tree_root(2, |i| leaves[i]);
        assert_eq!(root, (1.0e8f32 + 1.0) + (-1.0e8f32 + 1.0));
    }

    #[test]
    fn sampling_never_returns_a_zero_probability_state() {
        // GHZ on 5 qubits: only 00000 and 11111 have mass.
        let mut s = zero_state(5).unwrap();
        apply_matrix(&mut s, &Gate::H.matrix(), &[0], &[]);
        for q in 1..5 {
            apply_matrix(&mut s, &Gate::X.matrix(), &[q], &[0]);
        }
        let tree = SumTree::of_state(&s).unwrap();
        let shots = sample(&s, &tree, 42, 0, 4000).unwrap();
        let ones = shots.iter().filter(|&&o| o == 31).count();
        let zeros = shots.iter().filter(|&&o| o == 0).count();
        assert_eq!(ones + zeros, 4000);
        // Binomial(4000, 1/2): 5 sigma is about 158.
        assert!((ones as i64 - 2000).abs() < 158, "ones = {ones}");
    }

    #[test]
    fn sampling_is_a_function_of_seed_stream_and_shot_index() {
        let s = test_state(6, 5);
        let tree = SumTree::of_state(&s).unwrap();
        let a = sample(&s, &tree, 9, 1, 300).unwrap();
        let b = sample(&s, &tree, 9, 1, 100).unwrap();
        assert_eq!(&a[..100], &b[..]);
        assert_ne!(a, sample(&s, &tree, 9, 2, 300).unwrap());
        assert_ne!(a, sample(&s, &tree, 10, 1, 300).unwrap());
    }

    #[test]
    fn sampling_rejects_a_zero_state() {
        let s = vec![0.0f32; 8];
        let tree = SumTree::of_state(&s).unwrap();
        assert_eq!(sample(&s, &tree, 1, 0, 1).unwrap_err(), QError::ZeroNorm);
    }

    #[test]
    fn pauli_expectations_of_known_states() {
        let e = |s: &[f32], text: &str, n: u32| {
            expectation_pauli(s, &PauliString::parse(n, text).unwrap())
        };
        // |0>: <Z> = 1, <X> = <Y> = 0.
        let s = zero_state(1).unwrap();
        assert_eq!(e(&s, "Z0", 1), 1.0);
        assert_eq!(e(&s, "X0", 1), 0.0);
        assert_eq!(e(&s, "Y0", 1), 0.0);
        // |+>: <X> = 1 up to the rounding of 1/sqrt(2).
        let mut s = zero_state(1).unwrap();
        apply_matrix(&mut s, &Gate::H.matrix(), &[0], &[]);
        assert!((e(&s, "X0", 1) - 1.0).abs() < 1e-6);
        // S|+> = (|0> + i|1>)/sqrt(2): <Y> = 1.
        apply_matrix(&mut s, &Gate::S.matrix(), &[0], &[]);
        assert!((e(&s, "Y0", 1) - 1.0).abs() < 1e-6);
        assert!(e(&s, "X0", 1).abs() < 1e-6);
        // Bell state: <ZZ> = <XX> = 1, <YY> = -1, <Z0> = 0.
        let mut s = zero_state(2).unwrap();
        apply_matrix(&mut s, &Gate::H.matrix(), &[0], &[]);
        apply_matrix(&mut s, &Gate::X.matrix(), &[1], &[0]);
        assert!((e(&s, "Z0 Z1", 2) - 1.0).abs() < 1e-6);
        assert!((e(&s, "X0 X1", 2) - 1.0).abs() < 1e-6);
        assert!((e(&s, "Y0 Y1", 2) + 1.0).abs() < 1e-6);
        assert!(e(&s, "Z0", 2).abs() < 1e-6);
        assert!((e(&s, "I", 2) - 1.0).abs() < 1e-6);
    }
}
