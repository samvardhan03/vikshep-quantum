//! Correctness of the CPU reference against an independent simulator.
//!
//! The oracle below is written from the textbook definitions in binary64:
//! its own gate matrices, and a gate application that builds a new state
//! from the definition of a controlled operator (no in-place update, no
//! shared code with the kernels under test). Bit identity says the engine
//! is reproducible; these tests say it computes the right thing.

use vikshep_detmath::{cos, sin};
use vikshep_numerics::rng::Stream;
use vikshep_qcore::{C32, Circuit, Gate, Op, Pauli, PauliString, StateBackend};
use vikshep_statevec::{CpuBackend, Simulator};

type C = (f64, f64);

fn cmul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn cadd(a: C, b: C) -> C {
    (a.0 + b.0, a.1 + b.1)
}

fn scale(a: C, s: f64) -> C {
    (a.0 * s, a.1 * s)
}

/// `e^{i t}`.
fn expi(t: f64) -> C {
    (cos(t), sin(t))
}

const ZERO: C = (0.0, 0.0);
const ONE: C = (1.0, 0.0);
const I: C = (0.0, 1.0);
const NEG_I: C = (0.0, -1.0);

/// Textbook matrices in binary64.
fn matrix64(gate: &Gate) -> Vec<C> {
    let r = std::f64::consts::FRAC_1_SQRT_2;
    match gate {
        Gate::I => vec![ONE, ZERO, ZERO, ONE],
        Gate::X => vec![ZERO, ONE, ONE, ZERO],
        Gate::Y => vec![ZERO, NEG_I, I, ZERO],
        Gate::Z => vec![ONE, ZERO, ZERO, (-1.0, 0.0)],
        Gate::H => vec![(r, 0.0), (r, 0.0), (r, 0.0), (-r, 0.0)],
        Gate::S => vec![ONE, ZERO, ZERO, I],
        Gate::Sdg => vec![ONE, ZERO, ZERO, NEG_I],
        Gate::T => vec![ONE, ZERO, ZERO, expi(std::f64::consts::FRAC_PI_4)],
        Gate::Tdg => vec![ONE, ZERO, ZERO, expi(-std::f64::consts::FRAC_PI_4)],
        // sqrt(X) = (1/2) [[1 + i, 1 - i], [1 - i, 1 + i]].
        Gate::Sx => vec![(0.5, 0.5), (0.5, -0.5), (0.5, -0.5), (0.5, 0.5)],
        // exp(-i t X / 2) = cos(t/2) I - i sin(t/2) X.
        Gate::Rx(t) => {
            let (c, s) = (cos(t / 2.0), sin(t / 2.0));
            vec![(c, 0.0), (0.0, -s), (0.0, -s), (c, 0.0)]
        }
        // exp(-i t Y / 2) = cos(t/2) I - i sin(t/2) Y.
        Gate::Ry(t) => {
            let (c, s) = (cos(t / 2.0), sin(t / 2.0));
            vec![(c, 0.0), (-s, 0.0), (s, 0.0), (c, 0.0)]
        }
        // exp(-i t Z / 2) = diag(e^{-i t/2}, e^{i t/2}).
        Gate::Rz(t) => vec![expi(-t / 2.0), ZERO, ZERO, expi(t / 2.0)],
        Gate::P(l) => vec![ONE, ZERO, ZERO, expi(*l)],
        Gate::U(t, p, l) => {
            let (c, s) = (cos(t / 2.0), sin(t / 2.0));
            vec![
                (c, 0.0),
                scale(expi(*l), -s),
                scale(expi(*p), s),
                scale(expi(p + l), c),
            ]
        }
        Gate::Swap => {
            let mut m = vec![ZERO; 16];
            for (row, col) in [(0, 0), (1, 2), (2, 1), (3, 3)] {
                m[row * 4 + col] = ONE;
            }
            m
        }
        Gate::Unitary { matrix, .. } => matrix
            .iter()
            .map(|z| (f64::from(z.re), f64::from(z.im)))
            .collect(),
    }
}

/// Apply a (controlled) operator by its definition: amplitude `i` of the
/// result is the sum over target values of the matrix entry times the
/// amplitude with those target values, if every control bit of `i` is 1.
fn apply64(state: &[C], matrix: &[C], targets: &[u32], controls: &[u32]) -> Vec<C> {
    let dim = 1usize << targets.len();
    (0..state.len())
        .map(|i| {
            if controls.iter().any(|&c| (i >> c) & 1 == 0) {
                return state[i];
            }
            let row = targets
                .iter()
                .enumerate()
                .fold(0, |r, (b, &t)| r | (((i >> t) & 1) << b));
            let cleared = targets.iter().fold(i, |v, &t| v & !(1usize << t));
            (0..dim).fold(ZERO, |acc, col| {
                let j = targets
                    .iter()
                    .enumerate()
                    .fold(cleared, |v, (b, &t)| v | (((col >> b) & 1) << t));
                cadd(acc, cmul(matrix[row * dim + col], state[j]))
            })
        })
        .collect()
}

fn run64(circuit: &Circuit) -> Vec<C> {
    let mut state = vec![ZERO; 1usize << circuit.n_qubits()];
    state[0] = ONE;
    for op in circuit.ops() {
        state = apply64(&state, &matrix64(&op.gate), &op.targets, &op.controls);
    }
    state
}

fn widen(state: &[f32]) -> Vec<C> {
    state
        .chunks_exact(2)
        .map(|a| (f64::from(a[0]), f64::from(a[1])))
        .collect()
}

fn max_diff(a: &[C], b: &[C]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x.0 - y.0).abs().max((x.1 - y.1).abs()))
        .fold(0.0, f64::max)
}

/// `count` distinct qubits below `n`.
fn pick(stream: &mut Stream, n: u32, count: usize) -> Vec<u32> {
    stream
        .permutation(n as usize)
        .into_iter()
        .take(count)
        .map(|q| q as u32)
        .collect()
}

fn angle(stream: &mut Stream) -> f64 {
    (stream.next_f64_unit() * 2.0 - 1.0) * std::f64::consts::TAU
}

/// A random circuit over the whole gate set, including controlled gates and
/// dense matrices on up to three targets.
fn random_circuit(n: u32, depth: usize, seed: u64) -> Circuit {
    let mut s = Stream::new(seed, 100);
    let mut c = Circuit::new(n).unwrap();
    while c.len() < depth {
        let kind = s.uniform_below(20);
        let gate = match kind {
            0 => Gate::I,
            1 => Gate::X,
            2 => Gate::Y,
            3 => Gate::Z,
            4 => Gate::H,
            5 => Gate::S,
            6 => Gate::Sdg,
            7 => Gate::T,
            8 => Gate::Tdg,
            9 => Gate::Sx,
            10 => Gate::Rx(angle(&mut s)),
            11 => Gate::Ry(angle(&mut s)),
            12 => Gate::Rz(angle(&mut s)),
            13 => Gate::P(angle(&mut s)),
            14 | 15 => Gate::U(angle(&mut s), angle(&mut s), angle(&mut s)),
            16 | 17 => Gate::Swap,
            _ => {
                // A product of rotations is unitary, so norms stay near 1.
                let k = 1 + s.uniform_below(3) as u32;
                let mut m = vec![ZERO; 1usize << (2 * k)];
                for (d, slot) in m.iter_mut().step_by((1usize << k) + 1).enumerate() {
                    *slot = expi(angle(&mut s) + d as f64);
                }
                Gate::Unitary {
                    k,
                    matrix: m.iter().map(|z| C32::new(z.0 as f32, z.1 as f32)).collect(),
                }
            }
        };
        let k = gate.num_targets() as usize;
        if k > n as usize {
            continue;
        }
        let n_controls = (s.uniform_below(3) as usize).min(n as usize - k);
        let qubits = pick(&mut s, n, k + n_controls);
        c.push(Op::new(gate, &qubits[..k], &qubits[k..])).unwrap();
    }
    c
}

#[test]
fn random_circuits_match_the_binary64_oracle() {
    let mut worst = 0.0f64;
    for n in 1..=7u32 {
        for seed in 0..6u64 {
            let circuit = random_circuit(n, 80, 1000 * u64::from(n) + seed);
            let mut sim = Simulator::new(n).unwrap();
            sim.run(&circuit).unwrap();
            let diff = max_diff(&widen(&sim.state().unwrap()), &run64(&circuit));
            assert!(diff < 2e-5, "n = {n}, seed = {seed}: max difference {diff}");
            worst = worst.max(diff);
            let norm = f64::from(sim.norm_sq().unwrap());
            assert!(
                (norm - 1.0).abs() < 1e-4,
                "n = {n}, seed = {seed}: norm {norm}"
            );
        }
    }
    println!("largest amplitude difference from the oracle: {worst:e}");
}

#[test]
fn quantum_fourier_transform_matches_its_closed_form() {
    // QFT |x> = N^{-1/2} sum_k e^{2 pi i x k / N} |k>.
    let n = 5u32;
    let x = 11usize;
    let mut c = Circuit::new(n).unwrap();
    for q in 0..n {
        if (x >> q) & 1 == 1 {
            c.x(q).unwrap();
        }
    }
    for j in (0..n).rev() {
        c.h(j).unwrap();
        for k in (0..j).rev() {
            let theta = std::f64::consts::PI / f64::from(1u32 << (j - k));
            c.cp(theta, k, j).unwrap();
        }
    }
    for q in 0..n / 2 {
        c.swap(q, n - 1 - q).unwrap();
    }
    let mut sim = Simulator::new(n).unwrap();
    sim.run(&c).unwrap();
    let size = 1usize << n;
    let amp = 1.0 / (size as f64).sqrt();
    let expected: Vec<C> = (0..size)
        .map(|k| {
            let phase = std::f64::consts::TAU * ((x * k) % size) as f64 / size as f64;
            scale(expi(phase), amp)
        })
        .collect();
    let diff = max_diff(&widen(&sim.state().unwrap()), &expected);
    assert!(diff < 5e-6, "max difference {diff}");
}

#[test]
fn pauli_expectations_match_the_oracle() {
    let mut s = Stream::new(77, 0);
    for n in 1..=6u32 {
        let circuit = random_circuit(n, 40, 500 + u64::from(n));
        let mut sim = Simulator::new(n).unwrap();
        sim.run(&circuit).unwrap();
        let psi = widen(&sim.state().unwrap());
        for _ in 0..40 {
            let factors: Vec<(u32, Pauli)> = (0..n)
                .map(|q| {
                    let p = match s.uniform_below(4) {
                        0 => Pauli::I,
                        1 => Pauli::X,
                        2 => Pauli::Y,
                        _ => Pauli::Z,
                    };
                    (q, p)
                })
                .collect();
            let pauli = PauliString::new(n, &factors).unwrap();
            // <psi| P |psi> with P applied one factor at a time by the oracle.
            let mut p_psi = psi.clone();
            for &(q, p) in &factors {
                let gate = match p {
                    Pauli::I => continue,
                    Pauli::X => Gate::X,
                    Pauli::Y => Gate::Y,
                    Pauli::Z => Gate::Z,
                };
                p_psi = apply64(&p_psi, &matrix64(&gate), &[q], &[]);
            }
            let expected: f64 = psi
                .iter()
                .zip(&p_psi)
                .map(|(a, b)| a.0 * b.0 + a.1 * b.1)
                .sum();
            let got = f64::from(sim.expectation(&pauli).unwrap());
            assert!(
                (got - expected).abs() < 5e-6,
                "n = {n}, {}: got {got}, expected {expected}",
                pauli.canonical()
            );
        }
    }
}

#[test]
fn samples_follow_the_probabilities() {
    let n = 4u32;
    let circuit = random_circuit(n, 30, 4242);
    let mut sim = Simulator::new(n).unwrap();
    sim.run(&circuit).unwrap();
    let probs = sim.probabilities().unwrap();
    let total: f64 = probs.iter().map(|&p| f64::from(p)).sum();
    let shots = 200_000u64;
    let mut counts = vec![0u64; 1usize << n];
    for outcome in sim.sample(2024, 3, shots).unwrap() {
        counts[outcome as usize] += 1;
    }
    for (i, &count) in counts.iter().enumerate() {
        let p = f64::from(probs[i]) / total;
        let mean = p * shots as f64;
        let sigma = (shots as f64 * p * (1.0 - p)).sqrt();
        assert!(
            (count as f64 - mean).abs() <= 5.0 * sigma + 1.0,
            "outcome {i}: {count} draws, expected {mean:.1} +- {sigma:.1}"
        );
    }
}

#[test]
fn thread_count_does_not_change_a_single_bit() {
    let n = 18u32;
    let circuit = random_circuit(n, 24, 9001);
    let oid_with = |threads: usize| {
        let backend = CpuBackend::new(n).unwrap().with_threads(threads);
        assert_eq!(backend.threads(), threads);
        let mut sim = Simulator::with_backend(backend, n).unwrap();
        sim.run(&circuit).unwrap();
        (
            sim.state_oid().unwrap(),
            sim.sample(5, 0, 64).unwrap(),
            sim.expectation_str("X0 Y7 Z17").unwrap().to_bits(),
        )
    };
    let one = oid_with(1);
    assert_eq!(one, oid_with(2));
    assert_eq!(one, oid_with(8));
    assert_eq!(one, oid_with(5));
}

#[test]
fn default_measurement_methods_match_the_cpu_overrides() {
    // A backend that implements only the required methods, so the trait's
    // default measurement code runs.
    struct Minimal(CpuBackend);
    impl StateBackend for Minimal {
        fn name(&self) -> &str {
            "minimal"
        }
        fn n_qubits(&self) -> u32 {
            self.0.n_qubits()
        }
        fn reset(&mut self, n: u32) -> Result<(), vikshep_qcore::QError> {
            self.0.reset(n)
        }
        fn load(&mut self, n: u32, a: &[f32]) -> Result<(), vikshep_qcore::QError> {
            self.0.load(n, a)
        }
        fn apply(&mut self, m: &[C32], t: &[u32], c: &[u32]) -> Result<(), vikshep_qcore::QError> {
            self.0.apply(m, t, c)
        }
        fn read(&self) -> Result<Vec<f32>, vikshep_qcore::QError> {
            self.0.read()
        }
    }
    let n = 6u32;
    let circuit = random_circuit(n, 50, 31337);
    let mut a = Simulator::new(n).unwrap();
    a.run(&circuit).unwrap();
    let mut b = Simulator::with_backend(Minimal(CpuBackend::new(n).unwrap()), n).unwrap();
    b.run(&circuit).unwrap();
    let bits = |v: Vec<f32>| v.into_iter().map(f32::to_bits).collect::<Vec<_>>();
    assert_eq!(
        bits(a.probabilities().unwrap()),
        bits(b.probabilities().unwrap())
    );
    assert_eq!(
        a.norm_sq().unwrap().to_bits(),
        b.norm_sq().unwrap().to_bits()
    );
    assert_eq!(a.sample(1, 2, 500).unwrap(), b.sample(1, 2, 500).unwrap());
    assert_eq!(
        a.expectation_str("Y1 X4 Z5").unwrap().to_bits(),
        b.expectation_str("Y1 X4 Z5").unwrap().to_bits()
    );
}
