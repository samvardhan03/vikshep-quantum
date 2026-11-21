//! Cases of conformance suite v1 (`spec/VQS-1.md` section 11.2).
//!
//! Every input is derived from the Philox stream `(SEED, stream id)` of
//! VDS-1 section 5 by exact operations, so the inputs are part of the
//! specification and need no data files. Random amplitudes are
//! `(2 u - 1) * 2^-s` with `u = next_f32_unit()`: both steps are exact in
//! binary32.

use vikshep_numerics::oid::f32_le_bytes;
use vikshep_numerics::rng::Stream;
use vikshep_qcore::{C32, Circuit, Gate, Op, Pauli, PauliString, QError, StateBackend};
use vikshep_statevec::Simulator;

/// Master seed of every stream in the suite (ASCII "VQS1").
pub const SEED: u64 = 0x5651_5331;

/// One named output of a case, as canonical bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// Output name, unique within the case.
    pub name: &'static str,
    /// `float32`, `uint64` (both little-endian) or `utf8`.
    pub dtype: &'static str,
    /// Canonical bytes.
    pub bytes: Vec<u8>,
}

type Body = Box<dyn Fn(&mut dyn StateBackend) -> Result<Vec<Output>, QError> + Send + Sync>;

/// A conformance case: an identifier and a computation on a backend.
pub struct Case {
    /// Identifier, unique in the suite; its prefix names the family.
    pub id: String,
    body: Body,
}

impl Case {
    fn new(
        id: String,
        body: impl Fn(&mut dyn StateBackend) -> Result<Vec<Output>, QError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            body: Box::new(body),
        }
    }

    /// Run the case on `backend` and return its outputs.
    pub fn run(&self, backend: &mut dyn StateBackend) -> Result<Vec<Output>, QError> {
        (self.body)(backend)
    }
}

fn f32_out(name: &'static str, values: &[f32]) -> Output {
    Output {
        name,
        dtype: "float32",
        bytes: f32_le_bytes(values),
    }
}

fn u64_out(name: &'static str, values: &[u64]) -> Output {
    Output {
        name,
        dtype: "uint64",
        bytes: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
    }
}

fn utf8_out(name: &'static str, text: &str) -> Output {
    Output {
        name,
        dtype: "utf8",
        bytes: text.as_bytes().to_vec(),
    }
}

fn matrix_values(m: &[C32]) -> Vec<f32> {
    m.iter().flat_map(|z| [z.re, z.im]).collect()
}

/// Exact `2^-k` in binary32, `k <= 63`.
fn pow2_neg(k: u32) -> f32 {
    1.0 / (1u64 << k) as f32
}

/// A value in `[-1, 1)`, exact.
fn unit(s: &mut Stream) -> f32 {
    s.next_f32_unit() * 2.0 - 1.0
}

/// A random state of `n` qubits with components in `[-2^-s, 2^-s)`,
/// `s = ceil(n / 2)`. Not normalized.
fn random_state(n: u32, stream_id: u64) -> Vec<f32> {
    let mut s = Stream::new(SEED, stream_id);
    let scale = pow2_neg(n.div_ceil(2));
    (0..(2usize << n)).map(|_| unit(&mut s) * scale).collect()
}

/// A dense matrix on `k` targets with components in `[-2^-k, 2^-k)`.
fn random_matrix(k: u32, s: &mut Stream) -> Vec<C32> {
    let scale = pow2_neg(k);
    (0..(1usize << (2 * k)))
        .map(|_| {
            let re = unit(s) * scale;
            let im = unit(s) * scale;
            C32::new(re, im)
        })
        .collect()
}

/// `count` distinct qubits below `n`, in stream order.
fn pick(s: &mut Stream, n: u32, count: usize) -> Vec<u32> {
    s.permutation(n as usize)
        .into_iter()
        .take(count)
        .map(|q| q as u32)
        .collect()
}

/// An angle in `[-2 pi, 2 pi)`.
fn angle(s: &mut Stream) -> f64 {
    (s.next_f64_unit() * 2.0 - 1.0) * std::f64::consts::TAU
}

/// Load `state`, apply one operation, return the state.
fn one_op(
    backend: &mut dyn StateBackend,
    n: u32,
    state: &[f32],
    op: &Op,
) -> Result<Vec<Output>, QError> {
    let mut sim = Simulator::with_backend(backend, n)?;
    sim.load_state(state)?;
    sim.apply(op)?;
    Ok(vec![f32_out("state", &sim.state()?)])
}

// ---------------------------------------------------------------------------
// Family `matrix`: gate matrices built on the host (VQS-1 section 6).
// ---------------------------------------------------------------------------

const SPECIAL_ANGLES: [f64; 15] = [
    0.0,
    -0.0,
    std::f64::consts::PI,
    -std::f64::consts::PI,
    std::f64::consts::FRAC_PI_2,
    std::f64::consts::FRAC_PI_4,
    std::f64::consts::TAU,
    1.0e-300,
    1.0e-8,
    1.0,
    -1.0,
    1.0e6,
    123_456.789,
    f64::MIN_POSITIVE,
    5.0e-324,
];

fn matrix_cases(out: &mut Vec<Case>) {
    out.push(Case::new("matrix/fixed".into(), |_| {
        let gates = [
            Gate::I,
            Gate::X,
            Gate::Y,
            Gate::Z,
            Gate::H,
            Gate::S,
            Gate::Sdg,
            Gate::T,
            Gate::Tdg,
            Gate::Sx,
            Gate::Swap,
        ];
        let values: Vec<f32> = gates
            .iter()
            .flat_map(|g| matrix_values(&g.matrix()))
            .collect();
        Ok(vec![f32_out("matrices", &values)])
    }));

    type OneAngle = fn(f64) -> Gate;
    let one_angle: [(&str, OneAngle); 4] = [
        ("rx", Gate::Rx),
        ("ry", Gate::Ry),
        ("rz", Gate::Rz),
        ("p", Gate::P),
    ];
    for (index, (name, make)) in one_angle.into_iter().enumerate() {
        out.push(Case::new(format!("matrix/{name}"), move |_| {
            let mut s = Stream::new(SEED, 10 + index as u64);
            let mut values = Vec::new();
            for &a in &SPECIAL_ANGLES {
                values.extend(matrix_values(&make(a).matrix()));
            }
            for _ in 0..256 {
                let a = (s.next_f64_unit() * 2.0 - 1.0) * 25.132_741_228_718_345;
                values.extend(matrix_values(&make(a).matrix()));
            }
            Ok(vec![f32_out("matrices", &values)])
        }));
    }
    out.push(Case::new("matrix/u".into(), |_| {
        let mut s = Stream::new(SEED, 20);
        let mut values = Vec::new();
        let len = SPECIAL_ANGLES.len();
        for i in 0..len {
            let g = Gate::U(
                SPECIAL_ANGLES[i],
                SPECIAL_ANGLES[(i + 1) % len],
                SPECIAL_ANGLES[(i + 2) % len],
            );
            values.extend(matrix_values(&g.matrix()));
        }
        for _ in 0..256 {
            let theta = angle(&mut s);
            let phi = angle(&mut s);
            let lambda = angle(&mut s);
            values.extend(matrix_values(&Gate::U(theta, phi, lambda).matrix()));
        }
        Ok(vec![f32_out("matrices", &values)])
    }));
}

// ---------------------------------------------------------------------------
// Family `gate`: every named single-qubit gate on a random state.
// ---------------------------------------------------------------------------

fn named_single_qubit_gates() -> Vec<Gate> {
    vec![
        Gate::I,
        Gate::X,
        Gate::Y,
        Gate::Z,
        Gate::H,
        Gate::S,
        Gate::Sdg,
        Gate::T,
        Gate::Tdg,
        Gate::Sx,
        Gate::Rx(0.3),
        Gate::Ry(-1.2),
        Gate::Rz(2.5),
        Gate::P(0.7),
        Gate::U(1.1, -0.4, 2.2),
    ]
}

fn gate_cases(out: &mut Vec<Case>) {
    let mut stream_id = 1000u64;
    for gate in named_single_qubit_gates() {
        for (n, targets) in [(4u32, vec![0u32, 1, 2, 3]), (7, vec![0, 6])] {
            for t in targets {
                let op = Op::new(gate.clone(), &[t], &[]);
                let id = format!("gate/{}/n{n}/t{t}", gate.name());
                let sid = stream_id;
                stream_id += 1;
                out.push(Case::new(id, move |b| {
                    one_op(b, n, &random_state(n, sid), &op)
                }));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Family `ctrl`: controlled gates, swaps and multi-controlled gates.
// ---------------------------------------------------------------------------

/// `(label, n_qubits, gate, targets, controls)`.
type CtrlCase = (&'static str, u32, Gate, Vec<u32>, Vec<u32>);

fn ctrl_cases(out: &mut Vec<Case>) {
    let list: Vec<CtrlCase> = vec![
        ("cx-c0-t1", 4, Gate::X, vec![1], vec![0]),
        ("cx-c1-t0", 4, Gate::X, vec![0], vec![1]),
        ("cx-c3-t0", 4, Gate::X, vec![0], vec![3]),
        ("cx-c0-t3", 4, Gate::X, vec![3], vec![0]),
        ("cy-c2-t0", 4, Gate::Y, vec![0], vec![2]),
        ("cy-c0-t2", 4, Gate::Y, vec![2], vec![0]),
        ("cz-c2-t0", 4, Gate::Z, vec![0], vec![2]),
        ("cz-c0-t2", 4, Gate::Z, vec![2], vec![0]),
        ("ch-c2-t0", 4, Gate::H, vec![0], vec![2]),
        ("ch-c0-t2", 4, Gate::H, vec![2], vec![0]),
        ("cp-c2-t0", 4, Gate::P(0.9), vec![0], vec![2]),
        ("cp-c0-t2", 4, Gate::P(0.9), vec![2], vec![0]),
        ("crx-c2-t0", 4, Gate::Rx(1.3), vec![0], vec![2]),
        ("crx-c0-t2", 4, Gate::Rx(1.3), vec![2], vec![0]),
        ("cry-c2-t0", 4, Gate::Ry(-0.6), vec![0], vec![2]),
        ("cry-c0-t2", 4, Gate::Ry(-0.6), vec![2], vec![0]),
        ("crz-c2-t0", 4, Gate::Rz(2.1), vec![0], vec![2]),
        ("crz-c0-t2", 4, Gate::Rz(2.1), vec![2], vec![0]),
        ("cu-c1-t3", 4, Gate::U(0.4, 1.5, -0.8), vec![3], vec![1]),
        ("swap-0-1", 4, Gate::Swap, vec![0, 1], vec![]),
        ("swap-1-0", 4, Gate::Swap, vec![1, 0], vec![]),
        ("swap-0-3", 4, Gate::Swap, vec![0, 3], vec![]),
        ("swap-2-1", 4, Gate::Swap, vec![2, 1], vec![]),
        ("ccx-c01-t2", 5, Gate::X, vec![2], vec![0, 1]),
        ("ccx-c20-t1", 5, Gate::X, vec![1], vec![2, 0]),
        ("ccx-c31-t0", 5, Gate::X, vec![0], vec![3, 1]),
        ("ccx-c04-t2", 5, Gate::X, vec![2], vec![0, 4]),
        ("cswap-c0-12", 5, Gate::Swap, vec![1, 2], vec![0]),
        ("cswap-c3-20", 5, Gate::Swap, vec![2, 0], vec![3]),
        ("cswap-c2-40", 5, Gate::Swap, vec![4, 0], vec![2]),
        (
            "c3u-c024-t1",
            6,
            Gate::U(2.0, 0.3, 0.5),
            vec![1],
            vec![0, 2, 4],
        ),
        ("c4x-c0123-t5", 6, Gate::X, vec![5], vec![0, 1, 2, 3]),
        ("c4h-c5432-t0", 6, Gate::H, vec![0], vec![5, 4, 3, 2]),
        ("c5z-all", 6, Gate::Z, vec![3], vec![0, 1, 2, 4, 5]),
        ("ccswap-c05-31", 6, Gate::Swap, vec![3, 1], vec![0, 5]),
    ];
    for (index, (label, n, gate, targets, controls)) in list.into_iter().enumerate() {
        let op = Op::new(gate, &targets, &controls);
        let sid = 2000 + index as u64;
        out.push(Case::new(format!("ctrl/{label}"), move |b| {
            one_op(b, n, &random_state(n, sid), &op)
        }));
    }
}

// ---------------------------------------------------------------------------
// Family `unitary`: dense matrices on 1 to 6 targets, any target order.
// ---------------------------------------------------------------------------

fn unitary_cases(out: &mut Vec<Case>) {
    let mut index = 0u64;
    for (k, n, variants) in [
        (1u32, 6u32, 6u32),
        (2, 6, 6),
        (3, 6, 6),
        (4, 6, 6),
        (5, 7, 2),
        (6, 8, 1),
    ] {
        for v in 0..variants {
            let n_controls = ((v % 3) as usize).min((n - k) as usize);
            let sid = 3000 + index;
            index += 1;
            out.push(Case::new(format!("unitary/k{k}/n{n}/{v}"), move |b| {
                let mut s = Stream::new(SEED, sid);
                let matrix = random_matrix(k, &mut s);
                let qubits = pick(&mut s, n, k as usize + n_controls);
                let op = Op::new(
                    Gate::Unitary { k, matrix },
                    &qubits[..k as usize],
                    &qubits[k as usize..],
                );
                one_op(b, n, &random_state(n, sid + 500), &op)
            }));
        }
    }
}

// ---------------------------------------------------------------------------
// Family `edge`: values at the flush and rounding boundaries, signed zeros.
// ---------------------------------------------------------------------------

/// Bit patterns chosen to reach every branch of the Tier-1 arithmetic.
const EDGE_BITS: [u32; 24] = [
    0x0000_0000, // +0
    0x8000_0000, // -0
    0x0080_0000, // FLT_MIN
    0x8080_0000, // -FLT_MIN
    0x0080_0001, // next above FLT_MIN
    0x007F_FFFF, // largest subnormal (flushed on load)
    0x0000_0001, // smallest subnormal (flushed on load)
    0x8000_0001, // negative subnormal (flushed to -0 on load)
    0x0140_0000, // 3 * 2^-126
    0x2000_0000, // 2^-63: its square is exactly FLT_MIN
    0x2000_0001, // next above 2^-63
    0x1FFF_FFFF, // next below 2^-63: its square is below FLT_MIN
    0xA000_0000, // -2^-63
    0x1F80_0000, // 2^-64
    0x0D80_0000, // 2^-100
    0x3F7F_FFFF, // 1 - 2^-24: FLT_MIN times this rounds up to FLT_MIN
    0x3F80_0000, // 1
    0xBF80_0000, // -1
    0x3F80_0001, // 1 + 2^-23
    0x3F00_0000, // 0.5
    0x3F40_0000, // 0.75
    0x3F35_04F3, // 1 / sqrt(2)
    0x3080_0000, // 2^-30
    0xB080_0000, // -2^-30
];

fn edge_value(s: &mut Stream) -> f32 {
    f32::from_bits(EDGE_BITS[s.uniform_below(EDGE_BITS.len() as u64) as usize])
}

fn edge_cases(out: &mut Vec<Case>) {
    for (k, n, count) in [(1u32, 3u32, 40u64), (2, 4, 20)] {
        for v in 0..count {
            let sid = 4000 + u64::from(k) * 100 + v;
            out.push(Case::new(format!("edge/k{k}/{v}"), move |b| {
                let mut s = Stream::new(SEED, sid);
                let state: Vec<f32> = (0..(2usize << n)).map(|_| edge_value(&mut s)).collect();
                let matrix: Vec<C32> = (0..(1usize << (2 * k)))
                    .map(|_| {
                        let re = edge_value(&mut s);
                        let im = edge_value(&mut s);
                        C32::new(re, im)
                    })
                    .collect();
                let n_controls = (v % 2) as usize;
                let qubits = pick(&mut s, n, k as usize + n_controls);
                let op = Op::new(
                    Gate::Unitary { k, matrix },
                    &qubits[..k as usize],
                    &qubits[k as usize..],
                );
                one_op(b, n, &state, &op)
            }));
        }
    }
}

// ---------------------------------------------------------------------------
// Family `circuit`: whole circuits from |0...0>.
// ---------------------------------------------------------------------------

fn bell() -> Result<Circuit, QError> {
    let mut c = Circuit::new(2)?;
    c.h(0)?.cx(0, 1)?;
    Ok(c)
}

fn ghz(n: u32) -> Result<Circuit, QError> {
    let mut c = Circuit::new(n)?;
    c.h(0)?;
    for q in 1..n {
        c.cx(q - 1, q)?;
    }
    Ok(c)
}

/// Quantum Fourier transform of the basis state `x`.
fn qft(n: u32, x: u64) -> Result<Circuit, QError> {
    let mut c = Circuit::new(n)?;
    for q in 0..n {
        if (x >> q) & 1 == 1 {
            c.x(q)?;
        }
    }
    for j in (0..n).rev() {
        c.h(j)?;
        for k in (0..j).rev() {
            c.cp(std::f64::consts::PI / (1u64 << (j - k)) as f64, k, j)?;
        }
    }
    for q in 0..n / 2 {
        c.swap(q, n - 1 - q)?;
    }
    Ok(c)
}

/// A random circuit of named gates with up to two controls.
fn random_circuit(n: u32, depth: usize, stream_id: u64) -> Result<Circuit, QError> {
    let mut s = Stream::new(SEED, stream_id);
    let mut c = Circuit::new(n)?;
    while c.len() < depth {
        let gate = match s.uniform_below(18) {
            0 => Gate::I,
            1 => Gate::X,
            2 => Gate::Y,
            3 => Gate::Z,
            4 | 5 => Gate::H,
            6 => Gate::S,
            7 => Gate::Sdg,
            8 => Gate::T,
            9 => Gate::Tdg,
            10 => Gate::Sx,
            11 => Gate::Rx(angle(&mut s)),
            12 => Gate::Ry(angle(&mut s)),
            13 => Gate::Rz(angle(&mut s)),
            14 => Gate::P(angle(&mut s)),
            15 | 16 => {
                let theta = angle(&mut s);
                let phi = angle(&mut s);
                let lambda = angle(&mut s);
                Gate::U(theta, phi, lambda)
            }
            _ => Gate::Swap,
        };
        let k = gate.num_targets() as usize;
        if k > n as usize {
            continue;
        }
        let n_controls = (s.uniform_below(3) as usize).min(n as usize - k);
        let qubits = pick(&mut s, n, k + n_controls);
        c.push(Op::new(gate, &qubits[..k], &qubits[k..]))?;
    }
    Ok(c)
}

type CircuitMaker = Box<dyn Fn() -> Result<Circuit, QError> + Send + Sync>;

fn circuit_list() -> Vec<(String, CircuitMaker)> {
    let mut list: Vec<(String, CircuitMaker)> = vec![
        ("bell".into(), Box::new(bell)),
        ("ghz/n3".into(), Box::new(|| ghz(3))),
        ("ghz/n10".into(), Box::new(|| ghz(10))),
        ("qft/n4/x5".into(), Box::new(|| qft(4, 5))),
        ("qft/n8/x77".into(), Box::new(|| qft(8, 77))),
        ("qft/n11/x1234".into(), Box::new(|| qft(11, 1234))),
    ];
    for (index, (n, depth)) in [
        (1u32, 30usize),
        (2, 40),
        (4, 40),
        (4, 80),
        (8, 120),
        (8, 240),
        (12, 200),
        (14, 120),
        (16, 60),
    ]
    .into_iter()
    .enumerate()
    {
        let sid = 5000 + index as u64;
        list.push((
            format!("random/n{n}/d{depth}"),
            Box::new(move || random_circuit(n, depth, sid)),
        ));
    }
    list
}

fn circuit_cases(out: &mut Vec<Case>) {
    for (label, make) in circuit_list() {
        out.push(Case::new(format!("circuit/{label}"), move |b| {
            let circuit = make()?;
            let mut sim = Simulator::with_backend(b, circuit.n_qubits())?;
            sim.run(&circuit)?;
            let manifest = sim.manifest_value()?;
            let hash = match &manifest {
                vikshep_numerics::jcs::Value::Object(m) => match m.get("manifest_hash") {
                    Some(vikshep_numerics::jcs::Value::Str(h)) => h.clone(),
                    _ => return Err(QError::Backend("manifest has no hash".into())),
                },
                _ => return Err(QError::Backend("manifest is not an object".into())),
            };
            Ok(vec![
                utf8_out("circuit", &circuit.canonical_json()),
                f32_out("state", &sim.state()?),
                utf8_out("manifest_hash", &hash),
            ])
        }));
    }
}

// ---------------------------------------------------------------------------
// Families `measure` and `expect`: probabilities, sampling, expectation
// values (VQS-1 sections 7 to 9).
// ---------------------------------------------------------------------------

/// How a measurement case prepares its state.
#[derive(Clone)]
enum Prep {
    /// A random, unnormalized state.
    Random(u32, u64),
    /// A state loaded from explicit `(index, re, im)` entries; the rest is +0.
    Sparse(u32, Vec<(usize, f32, f32)>),
    /// A circuit from `|0...0>`.
    Ghz(u32),
    /// A random circuit from `|0...0>`.
    Circuit(u32, usize, u64),
}

impl Prep {
    fn n_qubits(&self) -> u32 {
        match self {
            Self::Random(n, _) | Self::Sparse(n, _) | Self::Ghz(n) | Self::Circuit(n, _, _) => *n,
        }
    }

    fn prepare<'a>(
        &self,
        backend: &'a mut dyn StateBackend,
    ) -> Result<Simulator<&'a mut dyn StateBackend>, QError> {
        let n = self.n_qubits();
        let mut sim = Simulator::with_backend(backend, n)?;
        match self {
            Self::Random(_, sid) => sim.load_state(&random_state(n, *sid))?,
            Self::Sparse(_, entries) => {
                let mut state = vec![0.0f32; 2usize << n];
                for &(i, re, im) in entries {
                    state[2 * i] = re;
                    state[2 * i + 1] = im;
                }
                sim.load_state(&state)?;
            }
            Self::Ghz(_) => sim.run(&ghz(n)?)?,
            Self::Circuit(_, depth, sid) => sim.run(&random_circuit(n, *depth, *sid)?)?,
        }
        Ok(sim)
    }
}

fn measurement_states() -> Vec<(String, Prep)> {
    let tiny = f32::MIN_POSITIVE;
    vec![
        ("random/n1".into(), Prep::Random(1, 6001)),
        ("random/n2".into(), Prep::Random(2, 6002)),
        ("random/n3".into(), Prep::Random(3, 6003)),
        ("random/n5".into(), Prep::Random(5, 6005)),
        ("random/n8".into(), Prep::Random(8, 6008)),
        ("random/n10".into(), Prep::Random(10, 6010)),
        (
            // Three occupied basis states among 64.
            "sparse/n6".into(),
            Prep::Sparse(6, vec![(0, 0.5, 0.0), (13, 0.0, -0.75), (63, 0.25, 0.25)]),
        ),
        (
            // Qubit 4 is always 0: the whole upper half has zero mass.
            "lower-half/n5".into(),
            Prep::Sparse(
                5,
                (0..16)
                    .map(|i| (i, 0.125 + i as f32 * 0.0625, -0.25))
                    .collect(),
            ),
        ),
        (
            // Probabilities that underflow next to ordinary ones.
            "tiny/n3".into(),
            Prep::Sparse(
                3,
                vec![
                    (1, 1.0e-19, 0.0),
                    (2, tiny, tiny),
                    (4, 1.0e-25, 1.0e-25),
                    (5, 0.5, 0.5),
                    (7, 1.0e-10, -1.0e-12),
                ],
            ),
        ),
        ("ghz/n10".into(), Prep::Ghz(10)),
        ("circuit/n6".into(), Prep::Circuit(6, 60, 6106)),
        ("circuit/n12".into(), Prep::Circuit(12, 100, 6112)),
    ]
}

fn measure_cases(out: &mut Vec<Case>) {
    for (index, (label, prep)) in measurement_states().into_iter().enumerate() {
        let stream = index as u64;
        out.push(Case::new(format!("measure/{label}"), move |b| {
            let sim = prep.prepare(b)?;
            Ok(vec![
                f32_out("probabilities", &sim.probabilities()?),
                f32_out("norm_sq", &[sim.norm_sq()?]),
                u64_out("samples", &sim.sample(0x00C0_FFEE + stream, stream, 512)?),
            ])
        }));
    }
}

fn expect_cases(out: &mut Vec<Case>) {
    for (index, (label, prep)) in measurement_states().into_iter().enumerate() {
        let sid = 7000 + index as u64;
        out.push(Case::new(format!("expect/{label}"), move |b| {
            let sim = prep.prepare(b)?;
            let n = sim.n_qubits();
            let mut s = Stream::new(SEED, sid);
            let mut strings = vec![PauliString::new(n, &[])?];
            for q in [0, n - 1] {
                for p in [Pauli::X, Pauli::Y, Pauli::Z] {
                    strings.push(PauliString::new(n, &[(q, p)])?);
                }
            }
            for _ in 0..24 {
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
                strings.push(PauliString::new(n, &factors)?);
            }
            let mut values = Vec::with_capacity(strings.len());
            for p in &strings {
                values.push(sim.expectation(p)?);
            }
            let text: Vec<String> = strings.iter().map(PauliString::canonical).collect();
            Ok(vec![
                utf8_out("strings", &text.join("\n")),
                f32_out("values", &values),
            ])
        }));
    }
}

/// Every case of suite v1, in report order.
#[must_use]
pub fn suite() -> Vec<Case> {
    let mut cases = Vec::new();
    matrix_cases(&mut cases);
    gate_cases(&mut cases);
    ctrl_cases(&mut cases);
    unitary_cases(&mut cases);
    edge_cases(&mut cases);
    circuit_cases(&mut cases);
    measure_cases(&mut cases);
    expect_cases(&mut cases);
    cases
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn case_identifiers_are_unique() {
        let cases = suite();
        let ids: BTreeSet<&str> = cases.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), cases.len());
        assert!(cases.len() > 200, "{} cases", cases.len());
    }

    #[test]
    fn random_inputs_are_exact() {
        // (2 u - 1) * 2^-s never rounds: every component is a multiple of
        // 2^-(23 + s) below 1 in magnitude.
        for &v in &random_state(5, 1) {
            let scaled = f64::from(v) * f64::from(1u32 << 26);
            assert_eq!(scaled, scaled.trunc());
            assert!(v.abs() <= 0.125);
        }
        assert_eq!(pow2_neg(0), 1.0);
        assert_eq!(pow2_neg(10), 1.0 / 1024.0);
    }

    #[test]
    fn edge_pool_is_what_the_comments_say() {
        assert_eq!(f32::from_bits(EDGE_BITS[2]), f32::MIN_POSITIVE);
        let half_min = f32::from_bits(0x2000_0000);
        assert_eq!(
            f64::from(half_min) * f64::from(half_min),
            f64::from(f32::MIN_POSITIVE)
        );
        assert_eq!(f32::from_bits(0x0140_0000), 3.0 * f32::MIN_POSITIVE);
        assert_eq!(f32::from_bits(0x0D80_0000), pow2_neg(50) * pow2_neg(50));
        assert_eq!(f32::from_bits(0x3080_0000), pow2_neg(30));
        assert_eq!(f32::from_bits(0x1F80_0000), pow2_neg(63) * 0.5);
        assert_eq!(f32::from_bits(0x3F35_04F3), vikshep_qcore::gates::INV_SQRT2);
    }
}
