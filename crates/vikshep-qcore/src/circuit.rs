//! Operations and circuits (`spec/VQS-1.md` sections 5 and 10).
//!
//! An [`Op`] is a gate, the target qubits it acts on and the control qubits
//! that condition it. A [`Circuit`] is a number of qubits and a list of
//! operations, applied one after another in list order. Operations are
//! never merged, reordered or cancelled: each is applied exactly as written,
//! because any rewriting changes the rounded result.
//!
//! The canonical form of a circuit is RFC 8785 JSON in which parameters and
//! matrix entries are written as bit patterns, so two circuits have the same
//! hash exactly when they prescribe the same arithmetic.

use vikshep_numerics::jcs::{Value, object};
use vikshep_numerics::oid::hex;

use crate::arith::C32;
use crate::error::QError;
use crate::gates::Gate;
use crate::{MAX_QUBITS, MAX_TARGETS, VQS_VERSION};

/// One operation: a gate on `targets`, applied where every qubit in
/// `controls` is 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    /// The gate.
    pub gate: Gate,
    /// Target qubits, in the order of the gate's matrix index bits.
    pub targets: Vec<u32>,
    /// Control qubits.
    pub controls: Vec<u32>,
}

impl Op {
    /// Build an operation (not yet checked against a number of qubits).
    #[must_use]
    pub fn new(gate: Gate, targets: &[u32], controls: &[u32]) -> Self {
        Self {
            gate,
            targets: targets.to_vec(),
            controls: controls.to_vec(),
        }
    }

    /// Check the operation against a state of `n_qubits` qubits.
    pub fn validate(&self, n_qubits: u32) -> Result<(), QError> {
        let k = self.gate.num_targets();
        if k == 0 || self.targets.len() != k as usize {
            return Err(QError::TargetCount {
                expected: k,
                got: self.targets.len(),
            });
        }
        if k > MAX_TARGETS {
            return Err(QError::TooManyTargets(k));
        }
        validate_qubits(n_qubits, &self.targets, &self.controls)?;
        if self.gate.params().iter().any(|p| !p.is_finite()) {
            return Err(QError::NonFinite("gate parameter"));
        }
        if let Gate::Unitary { matrix, .. } = &self.gate {
            validate_matrix(k, matrix)?;
        }
        Ok(())
    }

    fn value(&self) -> Value {
        let ints = |v: &[u32]| Value::Array(v.iter().map(|&q| Value::Int(i64::from(q))).collect());
        let mut entries = vec![
            ("controls", ints(&self.controls)),
            ("gate", Value::Str(self.gate.name().into())),
            (
                "params",
                Value::Array(
                    self.gate
                        .params()
                        .iter()
                        .map(|p| Value::Str(format!("{:016x}", p.to_bits())))
                        .collect(),
                ),
            ),
            ("targets", ints(&self.targets)),
        ];
        if let Gate::Unitary { .. } = &self.gate {
            let bytes: Vec<u8> = self
                .gate
                .matrix()
                .iter()
                .flat_map(|z| {
                    let mut b = [0u8; 8];
                    b[..4].copy_from_slice(&z.re.to_le_bytes());
                    b[4..].copy_from_slice(&z.im.to_le_bytes());
                    b
                })
                .collect();
            entries.push(("matrix", Value::Str(hex(&bytes))));
        }
        object(entries)
    }
}

/// Check target and control indices: in range and pairwise distinct.
pub fn validate_qubits(n_qubits: u32, targets: &[u32], controls: &[u32]) -> Result<(), QError> {
    let mut seen = 0u64;
    for &q in targets.iter().chain(controls) {
        if q >= n_qubits {
            return Err(QError::QubitOutOfRange { qubit: q, n_qubits });
        }
        let bit = 1u64 << q;
        if seen & bit != 0 {
            return Err(QError::DuplicateQubit(q));
        }
        seen |= bit;
    }
    Ok(())
}

/// Check a matrix on `k` targets: `4^k` finite entries.
pub fn validate_matrix(k: u32, matrix: &[C32]) -> Result<(), QError> {
    let expected = 1usize << (2 * k);
    if matrix.len() != expected {
        return Err(QError::MatrixSize {
            expected,
            got: matrix.len(),
        });
    }
    if matrix.iter().any(|z| !z.is_finite()) {
        return Err(QError::NonFinite("matrix entry"));
    }
    Ok(())
}

/// Check a number of qubits.
pub fn validate_n_qubits(n_qubits: u32) -> Result<(), QError> {
    if n_qubits == 0 || n_qubits > MAX_QUBITS {
        return Err(QError::QubitCount(n_qubits));
    }
    Ok(())
}

/// A list of operations on a fixed number of qubits.
#[derive(Clone, Debug, PartialEq)]
pub struct Circuit {
    n_qubits: u32,
    ops: Vec<Op>,
}

impl Circuit {
    /// An empty circuit on `n_qubits` qubits.
    pub fn new(n_qubits: u32) -> Result<Self, QError> {
        validate_n_qubits(n_qubits)?;
        Ok(Self {
            n_qubits,
            ops: Vec::new(),
        })
    }

    /// Number of qubits.
    #[must_use]
    pub fn n_qubits(&self) -> u32 {
        self.n_qubits
    }

    /// The operations, in application order.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// Number of operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the circuit has no operations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Append a checked operation.
    pub fn push(&mut self, op: Op) -> Result<&mut Self, QError> {
        op.validate(self.n_qubits)?;
        self.ops.push(op);
        Ok(self)
    }

    /// Append `gate` on `targets` with `controls`.
    pub fn gate(
        &mut self,
        gate: Gate,
        targets: &[u32],
        controls: &[u32],
    ) -> Result<&mut Self, QError> {
        self.push(Op::new(gate, targets, controls))
    }

    /// Identity on `q`.
    pub fn id(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::I, &[q], &[])
    }
    /// Pauli X on `q`.
    pub fn x(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::X, &[q], &[])
    }
    /// Pauli Y on `q`.
    pub fn y(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Y, &[q], &[])
    }
    /// Pauli Z on `q`.
    pub fn z(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Z, &[q], &[])
    }
    /// Hadamard on `q`.
    pub fn h(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::H, &[q], &[])
    }
    /// S on `q`.
    pub fn s(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::S, &[q], &[])
    }
    /// S-dagger on `q`.
    pub fn sdg(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Sdg, &[q], &[])
    }
    /// T on `q`.
    pub fn t(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::T, &[q], &[])
    }
    /// T-dagger on `q`.
    pub fn tdg(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Tdg, &[q], &[])
    }
    /// Square root of X on `q`.
    pub fn sx(&mut self, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Sx, &[q], &[])
    }
    /// X rotation by `theta` on `q`.
    pub fn rx(&mut self, theta: f64, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Rx(theta), &[q], &[])
    }
    /// Y rotation by `theta` on `q`.
    pub fn ry(&mut self, theta: f64, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Ry(theta), &[q], &[])
    }
    /// Z rotation by `theta` on `q`.
    pub fn rz(&mut self, theta: f64, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Rz(theta), &[q], &[])
    }
    /// Phase `lambda` on `q`.
    pub fn p(&mut self, lambda: f64, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::P(lambda), &[q], &[])
    }
    /// `U(theta, phi, lambda)` on `q`.
    pub fn u(&mut self, theta: f64, phi: f64, lambda: f64, q: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::U(theta, phi, lambda), &[q], &[])
    }
    /// Controlled X: control `c`, target `t`.
    pub fn cx(&mut self, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::X, &[t], &[c])
    }
    /// Controlled Y: control `c`, target `t`.
    pub fn cy(&mut self, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Y, &[t], &[c])
    }
    /// Controlled Z: control `c`, target `t`.
    pub fn cz(&mut self, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Z, &[t], &[c])
    }
    /// Controlled Hadamard: control `c`, target `t`.
    pub fn ch(&mut self, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::H, &[t], &[c])
    }
    /// Controlled phase `lambda`: control `c`, target `t`.
    pub fn cp(&mut self, lambda: f64, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::P(lambda), &[t], &[c])
    }
    /// Controlled X rotation: control `c`, target `t`.
    pub fn crx(&mut self, theta: f64, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Rx(theta), &[t], &[c])
    }
    /// Controlled Y rotation: control `c`, target `t`.
    pub fn cry(&mut self, theta: f64, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Ry(theta), &[t], &[c])
    }
    /// Controlled Z rotation: control `c`, target `t`.
    pub fn crz(&mut self, theta: f64, c: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Rz(theta), &[t], &[c])
    }
    /// Swap of `a` and `b` (targets `[a, b]`).
    pub fn swap(&mut self, a: u32, b: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Swap, &[a, b], &[])
    }
    /// Toffoli: controls `c0`, `c1`, target `t`.
    pub fn ccx(&mut self, c0: u32, c1: u32, t: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::X, &[t], &[c0, c1])
    }
    /// Controlled swap: control `c`, targets `[a, b]`.
    pub fn cswap(&mut self, c: u32, a: u32, b: u32) -> Result<&mut Self, QError> {
        self.gate(Gate::Swap, &[a, b], &[c])
    }
    /// An explicit matrix (`4^k` entries, row-major) on `targets`, with
    /// `controls`.
    pub fn unitary(
        &mut self,
        matrix: &[C32],
        targets: &[u32],
        controls: &[u32],
    ) -> Result<&mut Self, QError> {
        let k = u32::try_from(targets.len()).map_err(|_| QError::TooManyTargets(u32::MAX))?;
        self.gate(
            Gate::Unitary {
                k,
                matrix: matrix.to_vec(),
            },
            targets,
            controls,
        )
    }

    fn value(&self) -> Value {
        object([
            ("n_qubits", Value::Int(i64::from(self.n_qubits))),
            (
                "ops",
                Value::Array(self.ops.iter().map(Op::value).collect()),
            ),
            ("vqs_version", Value::Int(i64::from(VQS_VERSION))),
        ])
    }

    /// Canonical JSON of the circuit (VQS-1 section 10.1).
    #[must_use]
    pub fn canonical_json(&self) -> String {
        self.value()
            .canonical()
            .expect("circuit values are strings and small integers")
    }

    /// Lowercase hex SHA3-256 of [`Self::canonical_json`].
    #[must_use]
    pub fn hash(&self) -> String {
        self.value()
            .digest()
            .expect("circuit values are strings and small integers")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_form_is_stable_and_exact() {
        let mut c = Circuit::new(2).unwrap();
        c.h(0).unwrap().cx(0, 1).unwrap().rz(0.3, 1).unwrap();
        assert_eq!(
            c.canonical_json(),
            concat!(
                r#"{"n_qubits":2,"ops":["#,
                r#"{"controls":[],"gate":"h","params":[],"targets":[0]},"#,
                r#"{"controls":[0],"gate":"x","params":[],"targets":[1]},"#,
                r#"{"controls":[],"gate":"rz","params":["3fd3333333333333"],"targets":[1]}"#,
                r#"],"vqs_version":1}"#
            )
        );
        assert_eq!(c.hash().len(), 64);
    }

    #[test]
    fn signed_zero_parameters_have_different_hashes() {
        let mut a = Circuit::new(1).unwrap();
        a.rx(0.0, 0).unwrap();
        let mut b = Circuit::new(1).unwrap();
        b.rx(-0.0, 0).unwrap();
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn invalid_operations_are_rejected() {
        let mut c = Circuit::new(3).unwrap();
        assert_eq!(
            c.h(3).unwrap_err(),
            QError::QubitOutOfRange {
                qubit: 3,
                n_qubits: 3
            }
        );
        assert_eq!(c.cx(1, 1).unwrap_err(), QError::DuplicateQubit(1));
        assert_eq!(c.swap(2, 2).unwrap_err(), QError::DuplicateQubit(2));
        assert_eq!(
            c.rx(f64::NAN, 0).unwrap_err(),
            QError::NonFinite("gate parameter")
        );
        assert_eq!(
            c.unitary(&[C32::ONE; 3], &[0], &[]).unwrap_err(),
            QError::MatrixSize {
                expected: 4,
                got: 3
            }
        );
        assert_eq!(
            c.gate(Gate::X, &[0, 1], &[]).unwrap_err(),
            QError::TargetCount {
                expected: 1,
                got: 2
            }
        );
        assert!(c.is_empty());
        assert_eq!(Circuit::new(0).unwrap_err(), QError::QubitCount(0));
        assert_eq!(Circuit::new(41).unwrap_err(), QError::QubitCount(41));
    }
}
