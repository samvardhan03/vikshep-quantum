//! VQS-1 core: the deterministic arithmetic, gate matrices, circuit
//! description, reference kernels and backend trait of the Vikshep quantum
//! simulator (`spec/VQS-1.md`).
//!
//! * [`arith`]: Tier-1 binary32 operations and complex arithmetic (section 4).
//! * [`gates`]: gate matrices, built on the host and rounded once (section 6).
//! * [`circuit`]: operations, circuits and their canonical form (section 10).
//! * [`pauli`]: Pauli strings for expectation values (section 9).
//! * [`reference`]: the normative kernels over a host slice: gate
//!   application, probability tree, sampling and expectation values
//!   (sections 5, 7, 8 and 9).
//! * [`backend`]: the trait a backend implements to be tested for
//!   conformance (section 11).
//!
//! The arithmetic itself is the Vikshep open core's (`vikshep-numerics`,
//! `vikshep-detmath`), specified by VDS-1.

pub mod arith;
pub mod backend;
pub mod circuit;
pub mod error;
pub mod gates;
pub mod pauli;
pub mod reference;

pub use arith::C32;
pub use backend::StateBackend;
pub use circuit::{Circuit, Op};
pub use error::QError;
pub use gates::Gate;
pub use pauli::{Pauli, PauliString};

/// Version of the VQS-1 algorithms (gate application order, gate matrices,
/// probability tree, sampling, expectation values). See `spec/VQS-1.md`
/// section 12.
pub const VQS_VERSION: u32 = 1;

/// Version of the underlying Tier-1 arithmetic (VDS-1 section 10).
pub use vikshep_numerics::NUMERICS_VERSION;

/// Largest number of qubits a state may have (VQS-1 section 3.1).
pub const MAX_QUBITS: u32 = 40;

/// Largest number of target qubits of one operation in the reference
/// implementation (VQS-1 section 5.4).
pub const MAX_TARGETS: u32 = 8;

/// The README examples, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
