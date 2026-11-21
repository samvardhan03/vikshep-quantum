//! The `vikshep_quantum._core` extension module (PyO3).
//!
//! Arrays are converted with numpy (`numpy.asarray(obj, dtype=complex64)`)
//! and copied element by element, in logical row-major order, into buffers
//! owned by Rust before any computation, so the strides, memory order and
//! byte order of the caller's arrays never change a result. Conversion from
//! complex128 rounds each component once to binary32 (round to nearest,
//! ties to even).

use numpy::{AllowTypeChange, Complex32, PyArray1, PyArrayLikeDyn};
use pyo3::exceptions::{PyMemoryError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use vikshep_numerics::oid::oid as oid_of;
use vikshep_numerics::provenance::PLATFORM;
use vikshep_qcore::{C32, Circuit, Gate, MAX_QUBITS, NUMERICS_VERSION, Op, QError, VQS_VERSION};
use vikshep_statevec::{CpuBackend, Simulator};

type ComplexIn<'py> = PyArrayLikeDyn<'py, Complex32, AllowTypeChange>;

/// An operation as `(name, params, targets, controls)`.
type OpTuple = (String, Vec<f64>, Vec<u32>, Vec<u32>);

fn q_err(e: QError) -> PyErr {
    match e {
        QError::OutOfMemory(_) => PyMemoryError::new_err(e.to_string()),
        other => PyValueError::new_err(other.to_string()),
    }
}

fn value_err(msg: impl Into<String>) -> PyErr {
    PyValueError::new_err(msg.into())
}

/// Rust-owned row-major copy and shape of a complex array.
fn owned_complex(a: &ComplexIn<'_>) -> (Vec<C32>, Vec<usize>) {
    let view = a.as_array();
    (
        view.iter().map(|z| C32::new(z.re, z.im)).collect(),
        view.shape().to_vec(),
    )
}

/// A named gate with its parameters.
fn named_gate(name: &str, params: &[f64]) -> PyResult<Gate> {
    let lower = name.to_ascii_lowercase();
    let arity = match lower.as_str() {
        "rx" | "ry" | "rz" | "p" | "phase" => 1,
        "u" | "u3" => 3,
        _ => 0,
    };
    if params.len() != arity {
        return Err(value_err(format!(
            "gate {name:?} takes {arity} parameter(s), got {}",
            params.len()
        )));
    }
    Ok(match lower.as_str() {
        "id" | "i" => Gate::I,
        "x" => Gate::X,
        "y" => Gate::Y,
        "z" => Gate::Z,
        "h" => Gate::H,
        "s" => Gate::S,
        "sdg" => Gate::Sdg,
        "t" => Gate::T,
        "tdg" => Gate::Tdg,
        "sx" => Gate::Sx,
        "rx" => Gate::Rx(params[0]),
        "ry" => Gate::Ry(params[0]),
        "rz" => Gate::Rz(params[0]),
        "p" | "phase" => Gate::P(params[0]),
        "u" | "u3" => Gate::U(params[0], params[1], params[2]),
        "swap" => Gate::Swap,
        _ => {
            return Err(value_err(format!(
                "unknown gate {name:?}; use Circuit.unitary for an explicit matrix"
            )));
        }
    })
}

/// A quantum circuit: a number of qubits and a list of operations, applied
/// in order and never rewritten. Qubit 0 is the least significant bit of a
/// basis-state index. Gate methods return the circuit, so calls chain.
#[pyclass(module = "vikshep_quantum", name = "Circuit", skip_from_py_object)]
#[derive(Clone)]
struct PyCircuit {
    inner: Circuit,
}

fn push<'py>(
    mut slf: PyRefMut<'py, PyCircuit>,
    gate: Gate,
    targets: &[u32],
    controls: &[u32],
) -> PyResult<PyRefMut<'py, PyCircuit>> {
    slf.inner
        .push(Op::new(gate, targets, controls))
        .map_err(q_err)?;
    Ok(slf)
}

#[pymethods]
impl PyCircuit {
    #[new]
    fn new(n_qubits: u32) -> PyResult<Self> {
        Ok(Self {
            inner: Circuit::new(n_qubits).map_err(q_err)?,
        })
    }

    /// Number of qubits.
    #[getter]
    fn n_qubits(&self) -> u32 {
        self.inner.n_qubits()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "Circuit(n_qubits={}, ops={})",
            self.inner.n_qubits(),
            self.inner.len()
        )
    }

    /// Canonical JSON of the circuit (parameters as binary64 bit patterns).
    fn canonical_json(&self) -> String {
        self.inner.canonical_json()
    }

    /// Lowercase hex SHA3-256 of the canonical JSON.
    fn hash(&self) -> String {
        self.inner.hash()
    }

    /// An independent copy.
    fn copy(&self) -> Self {
        self.clone()
    }

    /// The operations as `(name, params, targets, controls)` tuples.
    fn ops(&self) -> Vec<OpTuple> {
        self.inner
            .ops()
            .iter()
            .map(|op| {
                (
                    op.gate.name().to_owned(),
                    op.gate.params(),
                    op.targets.clone(),
                    op.controls.clone(),
                )
            })
            .collect()
    }

    /// Append a named gate (`h`, `rx`, `u`, `swap`, ...) on `targets`,
    /// applied where every qubit in `controls` is 1.
    #[pyo3(signature = (name, targets, params = None, controls = None))]
    fn gate<'py>(
        slf: PyRefMut<'py, Self>,
        name: &str,
        targets: Vec<u32>,
        params: Option<Vec<f64>>,
        controls: Option<Vec<u32>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let gate = named_gate(name, &params.unwrap_or_default())?;
        push(slf, gate, &targets, &controls.unwrap_or_default())
    }

    /// Append an explicit `2^k x 2^k` matrix on `k` targets (row and column
    /// bit `m` is the value of `targets[m]`), with optional controls. The
    /// matrix is converted to complex64 and is not checked for unitarity.
    #[pyo3(signature = (matrix, targets, controls = None))]
    fn unitary<'py>(
        slf: PyRefMut<'py, Self>,
        matrix: ComplexIn<'py>,
        targets: Vec<u32>,
        controls: Option<Vec<u32>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (entries, shape) = owned_complex(&matrix);
        let k = u32::try_from(targets.len()).unwrap_or(u32::MAX);
        let dim = 1usize.checked_shl(k).unwrap_or(0);
        if shape != [dim, dim] {
            return Err(value_err(format!(
                "matrix for {k} target(s) must have shape ({dim}, {dim}), got {shape:?}"
            )));
        }
        push(
            slf,
            Gate::Unitary { k, matrix: entries },
            &targets,
            &controls.unwrap_or_default(),
        )
    }

    /// Identity on `q`.
    fn id(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::I, &[q], &[])
    }
    /// Pauli X on `q`.
    fn x(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::X, &[q], &[])
    }
    /// Pauli Y on `q`.
    fn y(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Y, &[q], &[])
    }
    /// Pauli Z on `q`.
    fn z(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Z, &[q], &[])
    }
    /// Hadamard on `q`.
    fn h(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::H, &[q], &[])
    }
    /// S = diag(1, i) on `q`.
    fn s(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::S, &[q], &[])
    }
    /// S-dagger on `q`.
    fn sdg(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Sdg, &[q], &[])
    }
    /// T = diag(1, e^{i pi/4}) on `q`.
    fn t(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::T, &[q], &[])
    }
    /// T-dagger on `q`.
    fn tdg(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Tdg, &[q], &[])
    }
    /// Square root of X on `q`.
    fn sx(slf: PyRefMut<'_, Self>, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Sx, &[q], &[])
    }
    /// Rotation about X by `theta` radians on `q`.
    fn rx(slf: PyRefMut<'_, Self>, theta: f64, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Rx(theta), &[q], &[])
    }
    /// Rotation about Y by `theta` radians on `q`.
    fn ry(slf: PyRefMut<'_, Self>, theta: f64, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Ry(theta), &[q], &[])
    }
    /// Rotation about Z by `theta` radians on `q`.
    fn rz(slf: PyRefMut<'_, Self>, theta: f64, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Rz(theta), &[q], &[])
    }
    /// Phase diag(1, e^{i lambda}) on `q`.
    fn p(slf: PyRefMut<'_, Self>, lam: f64, q: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::P(lam), &[q], &[])
    }
    /// U(theta, phi, lambda) of OpenQASM 3 on `q`.
    fn u(
        slf: PyRefMut<'_, Self>,
        theta: f64,
        phi: f64,
        lam: f64,
        q: u32,
    ) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::U(theta, phi, lam), &[q], &[])
    }
    /// Controlled X: control `c`, target `t`.
    fn cx(slf: PyRefMut<'_, Self>, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::X, &[t], &[c])
    }
    /// Controlled Y: control `c`, target `t`.
    fn cy(slf: PyRefMut<'_, Self>, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Y, &[t], &[c])
    }
    /// Controlled Z: control `c`, target `t`.
    fn cz(slf: PyRefMut<'_, Self>, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Z, &[t], &[c])
    }
    /// Controlled Hadamard: control `c`, target `t`.
    fn ch(slf: PyRefMut<'_, Self>, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::H, &[t], &[c])
    }
    /// Controlled phase `lam`: control `c`, target `t`.
    fn cp(slf: PyRefMut<'_, Self>, lam: f64, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::P(lam), &[t], &[c])
    }
    /// Controlled X rotation: control `c`, target `t`.
    fn crx(slf: PyRefMut<'_, Self>, theta: f64, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Rx(theta), &[t], &[c])
    }
    /// Controlled Y rotation: control `c`, target `t`.
    fn cry(slf: PyRefMut<'_, Self>, theta: f64, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Ry(theta), &[t], &[c])
    }
    /// Controlled Z rotation: control `c`, target `t`.
    fn crz(slf: PyRefMut<'_, Self>, theta: f64, c: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Rz(theta), &[t], &[c])
    }
    /// Swap of qubits `a` and `b`.
    fn swap(slf: PyRefMut<'_, Self>, a: u32, b: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Swap, &[a, b], &[])
    }
    /// Toffoli: controls `c0` and `c1`, target `t`.
    fn ccx(slf: PyRefMut<'_, Self>, c0: u32, c1: u32, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::X, &[t], &[c0, c1])
    }
    /// Controlled swap (Fredkin): control `c`, swapped qubits `a` and `b`.
    fn cswap(slf: PyRefMut<'_, Self>, c: u32, a: u32, b: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::Swap, &[a, b], &[c])
    }
    /// Multi-controlled X: `controls`, target `t`.
    fn mcx(slf: PyRefMut<'_, Self>, controls: Vec<u32>, t: u32) -> PyResult<PyRefMut<'_, Self>> {
        push(slf, Gate::X, &[t], &controls)
    }
}

/// A state-vector simulator on the CPU reference backend. The state starts
/// at |0...0>; results are bit-identical on every supported platform and
/// for every thread count.
#[pyclass(module = "vikshep_quantum", name = "Simulator")]
struct PySimulator {
    inner: Simulator<CpuBackend>,
}

#[pymethods]
impl PySimulator {
    #[new]
    #[pyo3(signature = (n_qubits, *, threads = None))]
    fn new(py: Python<'_>, n_qubits: u32, threads: Option<usize>) -> PyResult<Self> {
        let inner = py
            .detach(|| -> Result<_, QError> {
                let mut backend = CpuBackend::new(n_qubits)?;
                if let Some(t) = threads {
                    backend = backend.with_threads(t);
                }
                Simulator::with_backend(backend, n_qubits)
            })
            .map_err(q_err)?;
        Ok(Self { inner })
    }

    /// Number of qubits.
    #[getter]
    fn n_qubits(&self) -> u32 {
        self.inner.n_qubits()
    }

    /// Number of worker threads (results do not depend on it).
    #[getter]
    fn threads(&self) -> usize {
        self.inner.backend().threads()
    }

    /// The operations applied since the initial state, as a Circuit.
    #[getter]
    fn applied(&self) -> PyCircuit {
        PyCircuit {
            inner: self.inner.applied().clone(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Simulator(n_qubits={}, applied={}, threads={})",
            self.inner.n_qubits(),
            self.inner.applied().len(),
            self.inner.backend().threads()
        )
    }

    /// Return to |0...0> and forget the applied operations.
    fn reset(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.reset()).map_err(q_err)
    }

    /// Replace the state by `amplitudes` (2^n complex values, converted to
    /// complex64). Components are flushed to zero below FLT_MIN; the state
    /// is not normalized.
    fn load_state(&mut self, py: Python<'_>, amplitudes: ComplexIn<'_>) -> PyResult<()> {
        let (values, shape) = owned_complex(&amplitudes);
        let expected = 1usize << self.inner.n_qubits();
        if shape != [expected] {
            return Err(value_err(format!(
                "amplitudes must have shape ({expected},), got {shape:?}"
            )));
        }
        let flat: Vec<f32> = values.iter().flat_map(|z| [z.re, z.im]).collect();
        py.detach(|| self.inner.load_state(&flat)).map_err(q_err)
    }

    /// Apply every operation of `circuit`, in order.
    fn run(&mut self, py: Python<'_>, circuit: PyRef<'_, PyCircuit>) -> PyResult<()> {
        let circuit = circuit.inner.clone();
        py.detach(|| self.inner.run(&circuit)).map_err(q_err)
    }

    /// The state as a new complex64 array of 2^n amplitudes; index bit q is
    /// the value of qubit q.
    fn state<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<Complex32>>> {
        let state = self.inner.state().map_err(q_err)?;
        let values: Vec<Complex32> = state
            .chunks_exact(2)
            .map(|c| Complex32::new(c[0], c[1]))
            .collect();
        Ok(PyArray1::from_vec(py, values))
    }

    /// OID of the state's canonical bytes (28 hex characters).
    fn state_oid(&self) -> PyResult<String> {
        self.inner.state_oid().map_err(q_err)
    }

    /// Lowercase hex SHA3-256 of the state's canonical bytes.
    fn state_sha3(&self) -> PyResult<String> {
        self.inner.state_sha3().map_err(q_err)
    }

    /// Probability of every basis state (float32, not normalized).
    fn probabilities<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f32>>> {
        let p = py.detach(|| self.inner.probabilities()).map_err(q_err)?;
        Ok(PyArray1::from_vec(py, p))
    }

    /// Squared norm of the state, summed in the fixed pairwise order.
    fn norm_sq(&self) -> PyResult<f32> {
        self.inner.norm_sq().map_err(q_err)
    }

    /// Draw `shots` basis states (uint64; bit q is qubit q) from the Philox
    /// stream `(seed, stream)`. The same arguments give the same samples on
    /// every platform.
    #[pyo3(signature = (shots, seed, stream = 0))]
    fn sample<'py>(
        &self,
        py: Python<'py>,
        shots: u64,
        seed: u64,
        stream: u64,
    ) -> PyResult<Bound<'py, PyArray1<u64>>> {
        let out = py
            .detach(|| self.inner.sample(seed, stream, shots))
            .map_err(q_err)?;
        Ok(PyArray1::from_vec(py, out))
    }

    /// Expectation value of a Pauli string such as ``"X0 Z2 Y5"`` (``"I"``
    /// is the identity). The state is not normalized first.
    fn expectation(&self, py: Python<'_>, pauli: &str) -> PyResult<f32> {
        let pauli = pauli.to_owned();
        py.detach(|| self.inner.expectation_str(&pauli))
            .map_err(q_err)
    }

    /// The run manifest as canonical JSON. Its ``manifest_hash`` does not
    /// depend on the backend, platform or thread count.
    fn manifest(&self) -> PyResult<String> {
        self.inner.manifest().map_err(q_err)
    }
}

/// Run the whole VQS-1 conformance suite on the CPU reference against the
/// vectors compiled into the module.
#[pyfunction]
fn conformance_selftest<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let report = py
        .detach(|| -> Result<_, String> {
            let mut backend = CpuBackend::new(1).map_err(|e| e.to_string())?;
            vikshep_qconformance::runner::selftest(&mut backend)
        })
        .map_err(value_err)?;
    let out = PyDict::new(py);
    out.set_item("cases", report.cases.len())?;
    out.set_item("passed", report.passed())?;
    out.set_item("failed", report.failed())?;
    out.set_item("report", report.to_json())?;
    Ok(out)
}

/// OID of raw bytes: hex of the first 14 bytes of SHA3-256.
#[pyfunction]
fn oid(data: &[u8]) -> String {
    oid_of(data)
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("VQS_VERSION", VQS_VERSION)?;
    m.add("NUMERICS_VERSION", NUMERICS_VERSION)?;
    m.add("MAX_QUBITS", MAX_QUBITS)?;
    m.add("PLATFORM", PLATFORM)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<PyCircuit>()?;
    m.add_class::<PySimulator>()?;
    m.add_function(wrap_pyfunction!(conformance_selftest, m)?)?;
    m.add_function(wrap_pyfunction!(oid, m)?)?;
    Ok(())
}
