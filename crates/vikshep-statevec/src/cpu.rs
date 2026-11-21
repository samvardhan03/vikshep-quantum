//! The CPU reference backend.
//!
//! The state lives in one `Vec<f32>`. Gates run the reference kernel of
//! `vikshep-qcore`; on large states the vector is cut into equal contiguous
//! pieces, each closed under the operation, and the same kernel runs on
//! every piece in its own thread. The pieces are disjoint and the kernel
//! evaluates the same expressions on each amplitude group whatever the
//! piece, so the result has the same bytes for every thread count
//! (VQS-1 section 5.5).

use std::num::NonZero;
use std::thread;

use vikshep_qcore::reference::{self, SumTree};
use vikshep_qcore::{C32, PauliString, QError, StateBackend};

/// States below this many qubits are always processed on one thread: the
/// cost of starting threads exceeds the work.
const PARALLEL_MIN_QUBITS: u32 = 18;

/// The CPU reference backend.
#[derive(Clone, Debug)]
pub struct CpuBackend {
    n_qubits: u32,
    state: Vec<f32>,
    threads: usize,
}

impl CpuBackend {
    /// A backend holding `|0...0>` on `n_qubits` qubits, using as many
    /// threads as the machine reports.
    pub fn new(n_qubits: u32) -> Result<Self, QError> {
        let threads = thread::available_parallelism().map_or(1, NonZero::get);
        Ok(Self {
            n_qubits,
            state: reference::zero_state(n_qubits)?,
            threads,
        })
    }

    /// Set the number of worker threads (at least 1). Results do not depend
    /// on it.
    #[must_use]
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    /// Number of worker threads.
    #[must_use]
    pub fn threads(&self) -> usize {
        self.threads
    }

    /// The state, real and imaginary parts interleaved.
    #[must_use]
    pub fn state(&self) -> &[f32] {
        &self.state
    }

    /// Length of the contiguous pieces a gate may be split into, or `None`
    /// when the gate must run on one thread.
    fn piece_len(&self, targets: &[u32], controls: &[u32]) -> Option<usize> {
        if self.threads < 2 || self.n_qubits < PARALLEL_MIN_QUBITS {
            return None;
        }
        let top = targets.iter().chain(controls).copied().max()?;
        // A block of 2^(top + 1) amplitudes contains every amplitude group
        // of the operation that starts in it.
        let closed = 2usize << (top + 1);
        // Largest power of two not above the thread count.
        let workers = 1usize << (usize::BITS - 1 - self.threads.leading_zeros());
        let piece = (self.state.len() / workers).max(closed);
        (piece < self.state.len()).then_some(piece)
    }
}

impl StateBackend for CpuBackend {
    fn name(&self) -> &str {
        "cpu"
    }

    fn n_qubits(&self) -> u32 {
        self.n_qubits
    }

    fn reset(&mut self, n_qubits: u32) -> Result<(), QError> {
        if n_qubits == self.n_qubits {
            // Same size: reuse the buffer, so the state is never held twice.
            self.state.fill(0.0);
            self.state[0] = 1.0;
        } else {
            self.state = reference::zero_state(n_qubits)?;
            self.n_qubits = n_qubits;
        }
        Ok(())
    }

    fn load(&mut self, n_qubits: u32, amplitudes: &[f32]) -> Result<(), QError> {
        reference::check_state(n_qubits, amplitudes)?;
        if n_qubits == self.n_qubits {
            self.state.copy_from_slice(amplitudes);
        } else {
            let mut state = Vec::new();
            state
                .try_reserve_exact(amplitudes.len())
                .map_err(|_| QError::OutOfMemory(n_qubits))?;
            state.extend_from_slice(amplitudes);
            self.state = state;
            self.n_qubits = n_qubits;
        }
        reference::flush_state(&mut self.state);
        Ok(())
    }

    fn apply(&mut self, matrix: &[C32], targets: &[u32], controls: &[u32]) -> Result<(), QError> {
        match self.piece_len(targets, controls) {
            None => reference::apply_matrix(&mut self.state, matrix, targets, controls),
            Some(piece) => thread::scope(|scope| {
                for chunk in self.state.chunks_mut(piece) {
                    scope.spawn(move || reference::apply_matrix(chunk, matrix, targets, controls));
                }
            }),
        }
        Ok(())
    }

    fn read(&self) -> Result<Vec<f32>, QError> {
        Ok(self.state.clone())
    }

    fn probabilities(&self) -> Result<Vec<f32>, QError> {
        Ok(reference::probabilities(&self.state))
    }

    fn norm_sq(&self) -> Result<f32, QError> {
        Ok(reference::norm_sq_of(&self.state))
    }

    fn sample(&self, master_seed: u64, stream_id: u64, shots: u64) -> Result<Vec<u64>, QError> {
        let tree = SumTree::of_state(&self.state)?;
        reference::sample(&self.state, &tree, master_seed, stream_id, shots)
    }

    fn expectation(&self, pauli: &PauliString) -> Result<f32, QError> {
        if pauli.n_qubits() != self.n_qubits {
            return Err(QError::QubitMismatch {
                circuit: pauli.n_qubits(),
                state: self.n_qubits,
            });
        }
        Ok(reference::expectation_pauli(&self.state, pauli))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vikshep_qcore::Gate;

    #[test]
    fn piece_length_is_closed_under_the_operation() {
        let b = CpuBackend::new(20).unwrap().with_threads(8);
        let total = 2usize << 20;
        // Low qubits: eight equal pieces.
        assert_eq!(b.piece_len(&[0], &[3]), Some(total / 8));
        // Qubit 18 involved: pieces of 2^19 amplitudes, two of them.
        assert_eq!(b.piece_len(&[18], &[]), Some(total / 2));
        // Top qubit involved: one piece, so no split.
        assert_eq!(b.piece_len(&[19], &[]), None);
        assert_eq!(b.piece_len(&[0], &[19]), None);
        // One thread or a small state: no split.
        assert_eq!(b.clone().with_threads(1).piece_len(&[0], &[]), None);
        assert_eq!(
            CpuBackend::new(10)
                .unwrap()
                .with_threads(8)
                .piece_len(&[0], &[]),
            None
        );
        // Three threads round down to two pieces.
        assert_eq!(b.with_threads(3).piece_len(&[0], &[]), Some(total / 2));
    }

    #[test]
    fn load_flushes_and_rejects_bad_input() {
        let mut b = CpuBackend::new(1).unwrap();
        let tiny = f32::from_bits(1);
        b.load(1, &[tiny, -tiny, 0.5, 1.0]).unwrap();
        let bits: Vec<u32> = b.state().iter().map(|v| v.to_bits()).collect();
        assert_eq!(bits, [0, 0x8000_0000, 0.5f32.to_bits(), 1.0f32.to_bits()]);
        assert_eq!(
            b.load(1, &[0.0; 3]).unwrap_err(),
            QError::StateSize {
                expected: 4,
                got: 3
            }
        );
        assert_eq!(
            b.load(1, &[f32::NAN, 0.0, 0.0, 0.0]).unwrap_err(),
            QError::NonFinite("amplitude")
        );
        // A failed load leaves the state as it was.
        assert_eq!(b.state()[2], 0.5);
    }

    #[test]
    fn reset_restores_the_zero_state() {
        let mut b = CpuBackend::new(2).unwrap();
        b.apply(&Gate::H.matrix(), &[1], &[]).unwrap();
        b.reset(3).unwrap();
        assert_eq!(b.n_qubits(), 3);
        assert_eq!(b.state()[0], 1.0);
        assert!(b.state()[1..].iter().all(|v| v.to_bits() == 0));
    }
}
