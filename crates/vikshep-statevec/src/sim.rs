//! The simulator: circuits in, state and measurements out, with a record of
//! what was run (`spec/VQS-1.md` sections 5 to 10).

use vikshep_numerics::jcs::{Value, object};
use vikshep_numerics::oid::{f32_le_bytes, hex, oid, sha3_256};
use vikshep_numerics::provenance::PLATFORM;
use vikshep_qcore::reference;
use vikshep_qcore::{
    Circuit, NUMERICS_VERSION, Op, PauliString, QError, StateBackend, VQS_VERSION,
};

use crate::cpu::CpuBackend;

/// Value of the `schema` member of a run manifest.
pub const MANIFEST_SCHEMA: &str = "vikshep.quantum.run/1";

/// What the state was before the recorded operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InitialState {
    /// `|0...0>`.
    Zero,
    /// A state supplied by the caller, identified by the OID of its flushed
    /// bytes.
    Loaded {
        /// 28-character OID (VDS-1 section 9.2).
        oid: String,
    },
}

/// A state-vector simulator over a backend.
#[derive(Debug)]
pub struct Simulator<B: StateBackend = CpuBackend> {
    backend: B,
    initial: InitialState,
    /// Operations applied since the initial state, in order.
    applied: Circuit,
}

impl Simulator<CpuBackend> {
    /// A simulator on the CPU reference backend, in state `|0...0>`.
    pub fn new(n_qubits: u32) -> Result<Self, QError> {
        Self::with_backend(CpuBackend::new(n_qubits)?, n_qubits)
    }
}

impl<B: StateBackend> Simulator<B> {
    /// A simulator on `backend`, reset to `|0...0>` on `n_qubits` qubits.
    pub fn with_backend(mut backend: B, n_qubits: u32) -> Result<Self, QError> {
        let applied = Circuit::new(n_qubits)?;
        backend.reset(n_qubits)?;
        Ok(Self {
            backend,
            initial: InitialState::Zero,
            applied,
        })
    }

    /// Number of qubits.
    #[must_use]
    pub fn n_qubits(&self) -> u32 {
        self.applied.n_qubits()
    }

    /// The backend.
    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The initial state of the current run.
    #[must_use]
    pub fn initial_state(&self) -> &InitialState {
        &self.initial
    }

    /// The operations applied since the initial state.
    #[must_use]
    pub fn applied(&self) -> &Circuit {
        &self.applied
    }

    /// Return to `|0...0>` and forget the applied operations.
    pub fn reset(&mut self) -> Result<(), QError> {
        let n = self.n_qubits();
        self.backend.reset(n)?;
        self.initial = InitialState::Zero;
        self.applied = Circuit::new(n)?;
        Ok(())
    }

    /// Replace the state by `amplitudes` (`2 * 2^n` finite binary32 values,
    /// real and imaginary parts interleaved) and forget the applied
    /// operations. Components are flushed; the state is not normalized.
    pub fn load_state(&mut self, amplitudes: &[f32]) -> Result<(), QError> {
        let n = self.n_qubits();
        reference::check_state(n, amplitudes)?;
        let mut flushed = amplitudes.to_vec();
        reference::flush_state(&mut flushed);
        self.backend.load(n, &flushed)?;
        self.initial = InitialState::Loaded {
            oid: oid(&f32_le_bytes(&flushed)),
        };
        self.applied = Circuit::new(n)?;
        Ok(())
    }

    /// Apply one operation.
    pub fn apply(&mut self, op: &Op) -> Result<(), QError> {
        op.validate(self.n_qubits())?;
        self.backend
            .apply(&op.gate.matrix(), &op.targets, &op.controls)?;
        self.applied.push(op.clone())?;
        Ok(())
    }

    /// Apply every operation of `circuit`, in order. A circuit on a
    /// different number of qubits is rejected before anything is applied;
    /// the operations themselves were checked when the circuit was built.
    pub fn run(&mut self, circuit: &Circuit) -> Result<(), QError> {
        if circuit.n_qubits() != self.n_qubits() {
            return Err(QError::QubitMismatch {
                circuit: circuit.n_qubits(),
                state: self.n_qubits(),
            });
        }
        for op in circuit.ops() {
            self.apply(op)?;
        }
        Ok(())
    }

    /// The state: `2 * 2^n` binary32 values, amplitude `i` at `[2 i]` (real)
    /// and `[2 i + 1]` (imaginary).
    pub fn state(&self) -> Result<Vec<f32>, QError> {
        self.backend.read()
    }

    /// OID of the state's canonical bytes (VQS-1 section 10.2).
    pub fn state_oid(&self) -> Result<String, QError> {
        Ok(oid(&f32_le_bytes(&self.state()?)))
    }

    /// Lowercase hex SHA3-256 of the state's canonical bytes.
    pub fn state_sha3(&self) -> Result<String, QError> {
        Ok(hex(&sha3_256(&f32_le_bytes(&self.state()?))))
    }

    /// Probability of every basis state.
    pub fn probabilities(&self) -> Result<Vec<f32>, QError> {
        self.backend.probabilities()
    }

    /// Squared norm of the state.
    pub fn norm_sq(&self) -> Result<f32, QError> {
        self.backend.norm_sq()
    }

    /// Draw `shots` basis states from stream `(master_seed, stream_id)`.
    /// Bit `q` of an outcome is the value of qubit `q`.
    pub fn sample(&self, master_seed: u64, stream_id: u64, shots: u64) -> Result<Vec<u64>, QError> {
        self.backend.sample(master_seed, stream_id, shots)
    }

    /// Expectation value of a Pauli string. The state is not normalized
    /// first: for a state of squared norm `s` the result is `s` times the
    /// normalized expectation value.
    pub fn expectation(&self, pauli: &PauliString) -> Result<f32, QError> {
        self.backend.expectation(pauli)
    }

    /// Expectation value of a Pauli string in text form (`X0 Z2 Y5`).
    pub fn expectation_str(&self, pauli: &str) -> Result<f32, QError> {
        self.expectation(&PauliString::parse(self.n_qubits(), pauli)?)
    }

    /// The run manifest as a JSON value (VQS-1 section 10.3).
    pub fn manifest_value(&self) -> Result<Value, QError> {
        let n = self.n_qubits();
        let initial = match &self.initial {
            InitialState::Zero => object([("kind", Value::Str("zero".into()))]),
            InitialState::Loaded { oid } => object([
                ("kind", Value::Str("loaded".into())),
                ("oid", Value::Str(oid.clone())),
            ]),
        };
        let hashed = vec![
            ("schema", Value::Str(MANIFEST_SCHEMA.into())),
            ("vqs_version", Value::Int(i64::from(VQS_VERSION))),
            ("numerics_version", Value::Int(i64::from(NUMERICS_VERSION))),
            ("n_qubits", Value::Int(i64::from(n))),
            ("initial_state", initial),
            ("circuit_sha3_256", Value::Str(self.applied.hash())),
            ("n_ops", Value::Int(self.applied.len() as i64)),
            (
                "state",
                object([
                    ("dtype", Value::Str("float32".into())),
                    ("oid", Value::Str(self.state_oid()?)),
                    (
                        "shape",
                        Value::Array(vec![Value::Int(1i64 << n), Value::Int(2)]),
                    ),
                ]),
            ),
        ];
        let manifest_hash = object(hashed.clone())
            .digest()
            .map_err(|e| QError::Backend(e.to_string()))?;
        let mut all = hashed;
        all.push((
            "execution",
            object([
                ("backend", Value::Str(self.backend.name().into())),
                ("platform", Value::Str(PLATFORM.into())),
                (
                    "package_version",
                    Value::Str(env!("CARGO_PKG_VERSION").into()),
                ),
            ]),
        ));
        all.push(("manifest_hash", Value::Str(manifest_hash)));
        Ok(object(all))
    }

    /// The run manifest as canonical JSON. `manifest_hash` is the SHA3-256
    /// of the manifest without its `execution` and `manifest_hash` members,
    /// so the same run on any backend or platform has the same hash.
    pub fn manifest(&self) -> Result<String, QError> {
        self.manifest_value()?
            .canonical()
            .map_err(|e| QError::Backend(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vikshep_qcore::Gate;

    #[test]
    fn rejected_operations_do_not_change_the_state_or_the_record() {
        let mut sim = Simulator::new(2).unwrap();
        sim.apply(&Op::new(Gate::H, &[0], &[])).unwrap();
        let before = sim.state_oid().unwrap();
        assert!(sim.apply(&Op::new(Gate::X, &[2], &[])).is_err());
        assert!(sim.run(&Circuit::new(3).unwrap()).is_err());
        assert_eq!(sim.state_oid().unwrap(), before);
        assert_eq!(sim.applied().len(), 1);
    }

    #[test]
    fn manifest_hash_ignores_where_the_run_happened() {
        let mut c = Circuit::new(3).unwrap();
        c.h(0).unwrap().cx(0, 1).unwrap().rz(0.25, 2).unwrap();
        let mut a = Simulator::new(3).unwrap();
        a.run(&c).unwrap();
        let ma = a.manifest().unwrap();
        assert!(ma.contains(r#""schema":"vikshep.quantum.run/1""#));
        assert!(ma.contains(r#""n_ops":3"#));
        assert!(ma.contains(r#""shape":[8,2]"#));
        assert!(ma.contains(&format!(r#""circuit_sha3_256":"{}""#, c.hash())));

        // The same run through a boxed backend with another thread count.
        let boxed: Box<dyn StateBackend> = Box::new(CpuBackend::new(3).unwrap().with_threads(1));
        let mut b = Simulator::with_backend(boxed, 3).unwrap();
        b.run(&c).unwrap();
        let hash = |m: &str| {
            let at = m.find(r#""manifest_hash":""#).unwrap() + 17;
            m[at..at + 64].to_string()
        };
        assert_eq!(hash(&ma), hash(&b.manifest().unwrap()));

        // A different circuit, or a loaded initial state, changes the hash.
        let mut d = Simulator::new(3).unwrap();
        d.run(&c).unwrap();
        d.apply(&Op::new(Gate::I, &[0], &[])).unwrap();
        assert_ne!(hash(&ma), hash(&d.manifest().unwrap()));
        let mut e = Simulator::new(3).unwrap();
        e.load_state(&reference::zero_state(3).unwrap()).unwrap();
        e.run(&c).unwrap();
        assert_eq!(e.state_oid().unwrap(), a.state_oid().unwrap());
        assert_ne!(hash(&ma), hash(&e.manifest().unwrap()));
    }

    #[test]
    fn reset_and_load_clear_the_record() {
        let mut sim = Simulator::new(1).unwrap();
        sim.apply(&Op::new(Gate::X, &[0], &[])).unwrap();
        sim.reset().unwrap();
        assert!(sim.applied().is_empty());
        assert_eq!(sim.initial_state(), &InitialState::Zero);
        assert_eq!(sim.state().unwrap(), [1.0, 0.0, 0.0, 0.0]);
        sim.load_state(&[0.0, 0.0, 0.0, 1.0]).unwrap();
        assert!(matches!(sim.initial_state(), InitialState::Loaded { .. }));
        assert_eq!(sim.expectation_str("Z0").unwrap(), -1.0);
    }
}
