//! Python bindings of the Vikshep quantum simulator: the `vikshep-quantum`
//! distribution, imported as `vikshep_quantum` (PyO3 + maturin, abi3 wheels
//! for Python 3.10 and later).
//!
//! The extension module is compiled only with the `python` feature, which
//! maturin enables (`crates/vikshep-quantum-py/pyproject.toml`); without it
//! this crate is empty, so `cargo test --workspace` never links against
//! Python.

#[cfg(feature = "python")]
mod python;
