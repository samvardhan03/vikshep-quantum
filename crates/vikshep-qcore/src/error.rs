//! Errors of the VQS-1 core.

use std::fmt;

/// Why an operation was rejected. Every rejection happens before any
/// arithmetic, so a failed call never leaves a partially updated state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QError {
    /// The number of qubits is zero or above [`crate::MAX_QUBITS`].
    QubitCount(u32),
    /// A qubit index is not below the number of qubits.
    QubitOutOfRange {
        /// The offending index.
        qubit: u32,
        /// The number of qubits of the state or circuit.
        n_qubits: u32,
    },
    /// The same qubit appears twice among the targets and controls.
    DuplicateQubit(u32),
    /// The number of targets does not match the gate.
    TargetCount {
        /// Targets the gate acts on.
        expected: u32,
        /// Targets given.
        got: usize,
    },
    /// More targets than the reference implementation supports.
    TooManyTargets(u32),
    /// A matrix does not have `4^k` entries.
    MatrixSize {
        /// Entries required.
        expected: usize,
        /// Entries given.
        got: usize,
    },
    /// A parameter, matrix entry or amplitude is NaN or infinite.
    NonFinite(&'static str),
    /// A state buffer does not have `2 * 2^n` elements.
    StateSize {
        /// Elements required.
        expected: usize,
        /// Elements given.
        got: usize,
    },
    /// A circuit was run on a state with a different number of qubits.
    QubitMismatch {
        /// Qubits of the circuit.
        circuit: u32,
        /// Qubits of the state.
        state: u32,
    },
    /// Sampling needs a positive, finite squared norm (VQS-1 section 8).
    ZeroNorm,
    /// The state does not fit in memory.
    OutOfMemory(u32),
    /// A Pauli string could not be parsed.
    PauliSyntax(String),
    /// A backend-specific failure.
    Backend(String),
}

impl fmt::Display for QError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QubitCount(n) => write!(
                f,
                "number of qubits must be between 1 and {}, got {n}",
                crate::MAX_QUBITS
            ),
            Self::QubitOutOfRange { qubit, n_qubits } => {
                write!(f, "qubit {qubit} is out of range for {n_qubits} qubits")
            }
            Self::DuplicateQubit(q) => {
                write!(
                    f,
                    "qubit {q} appears more than once among targets and controls"
                )
            }
            Self::TargetCount { expected, got } => {
                write!(f, "gate acts on {expected} target qubits, got {got}")
            }
            Self::TooManyTargets(k) => write!(
                f,
                "{k} target qubits; the reference implementation supports at most {}",
                crate::MAX_TARGETS
            ),
            Self::MatrixSize { expected, got } => {
                write!(f, "matrix needs {expected} entries, got {got}")
            }
            Self::NonFinite(what) => write!(f, "{what} must be finite"),
            Self::StateSize { expected, got } => {
                write!(f, "state needs {expected} binary32 elements, got {got}")
            }
            Self::QubitMismatch { circuit, state } => {
                write!(f, "circuit has {circuit} qubits, state has {state}")
            }
            Self::ZeroNorm => write!(f, "squared norm is not positive and finite"),
            Self::OutOfMemory(n) => write!(f, "not enough memory for a {n}-qubit state"),
            Self::PauliSyntax(s) => write!(f, "Pauli string: {s}"),
            Self::Backend(s) => write!(f, "backend: {s}"),
        }
    }
}

impl std::error::Error for QError {}
