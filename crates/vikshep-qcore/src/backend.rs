//! The trait a state-vector backend implements (`spec/VQS-1.md` section 11).
//!
//! A backend owns one state and applies operations to it. Only `reset`,
//! `load`, `apply` and `read` are required; the measurement functions have
//! default implementations that copy the state to the host and run the
//! reference kernels. A backend MAY override them for speed, and its results
//! MUST then have the same bytes as the defaults.

use crate::arith::C32;
use crate::error::QError;
use crate::pauli::PauliString;
use crate::reference::{self, SumTree};

/// A state-vector backend.
pub trait StateBackend {
    /// Short backend name, for example `cpu`.
    fn name(&self) -> &str;

    /// Number of qubits of the current state.
    fn n_qubits(&self) -> u32;

    /// Replace the state by `|0...0>` on `n_qubits` qubits.
    fn reset(&mut self, n_qubits: u32) -> Result<(), QError>;

    /// Replace the state by `amplitudes` (`2 * 2^n` finite binary32 values,
    /// real and imaginary parts interleaved). The backend flushes every
    /// component and does not normalize.
    fn load(&mut self, n_qubits: u32, amplitudes: &[f32]) -> Result<(), QError>;

    /// Apply a flushed, validated matrix to `targets` where all `controls`
    /// are 1 (VQS-1 section 5).
    fn apply(&mut self, matrix: &[C32], targets: &[u32], controls: &[u32]) -> Result<(), QError>;

    /// Copy the state to the host.
    fn read(&self) -> Result<Vec<f32>, QError>;

    /// Probability of every basis state (VQS-1 section 7.1).
    fn probabilities(&self) -> Result<Vec<f32>, QError> {
        Ok(reference::probabilities(&self.read()?))
    }

    /// Squared norm: the root of the probability tree (VQS-1 section 7.2).
    fn norm_sq(&self) -> Result<f32, QError> {
        Ok(reference::norm_sq_of(&self.read()?))
    }

    /// Draw `shots` basis states (VQS-1 section 8).
    fn sample(&self, master_seed: u64, stream_id: u64, shots: u64) -> Result<Vec<u64>, QError> {
        let state = self.read()?;
        let tree = SumTree::of_state(&state)?;
        reference::sample(&state, &tree, master_seed, stream_id, shots)
    }

    /// Expectation value of a Pauli string (VQS-1 section 9).
    fn expectation(&self, pauli: &PauliString) -> Result<f32, QError> {
        if pauli.n_qubits() != self.n_qubits() {
            return Err(QError::QubitMismatch {
                circuit: pauli.n_qubits(),
                state: self.n_qubits(),
            });
        }
        Ok(reference::expectation_pauli(&self.read()?, pauli))
    }
}

impl<T: StateBackend + ?Sized> StateBackend for Box<T> {
    fn name(&self) -> &str {
        (**self).name()
    }
    fn n_qubits(&self) -> u32 {
        (**self).n_qubits()
    }
    fn reset(&mut self, n_qubits: u32) -> Result<(), QError> {
        (**self).reset(n_qubits)
    }
    fn load(&mut self, n_qubits: u32, amplitudes: &[f32]) -> Result<(), QError> {
        (**self).load(n_qubits, amplitudes)
    }
    fn apply(&mut self, matrix: &[C32], targets: &[u32], controls: &[u32]) -> Result<(), QError> {
        (**self).apply(matrix, targets, controls)
    }
    fn read(&self) -> Result<Vec<f32>, QError> {
        (**self).read()
    }
    fn probabilities(&self) -> Result<Vec<f32>, QError> {
        (**self).probabilities()
    }
    fn norm_sq(&self) -> Result<f32, QError> {
        (**self).norm_sq()
    }
    fn sample(&self, master_seed: u64, stream_id: u64, shots: u64) -> Result<Vec<u64>, QError> {
        (**self).sample(master_seed, stream_id, shots)
    }
    fn expectation(&self, pauli: &PauliString) -> Result<f32, QError> {
        (**self).expectation(pauli)
    }
}

impl<T: StateBackend + ?Sized> StateBackend for &mut T {
    fn name(&self) -> &str {
        (**self).name()
    }
    fn n_qubits(&self) -> u32 {
        (**self).n_qubits()
    }
    fn reset(&mut self, n_qubits: u32) -> Result<(), QError> {
        (**self).reset(n_qubits)
    }
    fn load(&mut self, n_qubits: u32, amplitudes: &[f32]) -> Result<(), QError> {
        (**self).load(n_qubits, amplitudes)
    }
    fn apply(&mut self, matrix: &[C32], targets: &[u32], controls: &[u32]) -> Result<(), QError> {
        (**self).apply(matrix, targets, controls)
    }
    fn read(&self) -> Result<Vec<f32>, QError> {
        (**self).read()
    }
    fn probabilities(&self) -> Result<Vec<f32>, QError> {
        (**self).probabilities()
    }
    fn norm_sq(&self) -> Result<f32, QError> {
        (**self).norm_sq()
    }
    fn sample(&self, master_seed: u64, stream_id: u64, shots: u64) -> Result<Vec<u64>, QError> {
        (**self).sample(master_seed, stream_id, shots)
    }
    fn expectation(&self, pauli: &PauliString) -> Result<f32, QError> {
        (**self).expectation(pauli)
    }
}
