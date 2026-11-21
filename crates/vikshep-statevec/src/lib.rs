//! The VQS-1 CPU reference state-vector simulator.
//!
//! * [`CpuBackend`]: the reference backend. It runs the normative kernels of
//!   `vikshep-qcore` and is the definition every other backend is compared
//!   with, byte for byte.
//! * [`Simulator`]: applies circuits to a backend, and reports the state,
//!   probabilities, samples, expectation values and a provenance manifest.
//!
//! ```
//! use vikshep_qcore::Circuit;
//! use vikshep_statevec::Simulator;
//!
//! let mut circuit = Circuit::new(2)?;
//! circuit.h(0)?.cx(0, 1)?;
//! let mut sim = Simulator::new(2)?;
//! sim.run(&circuit)?;
//! let shots = sim.sample(7, 0, 100)?;
//! assert!(shots.iter().all(|&s| s == 0b00 || s == 0b11));
//! # Ok::<(), vikshep_qcore::QError>(())
//! ```

pub mod cpu;
pub mod sim;

pub use cpu::CpuBackend;
pub use sim::{InitialState, Simulator};

/// The README examples, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
