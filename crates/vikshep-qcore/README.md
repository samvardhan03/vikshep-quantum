# vikshep-qcore

The core of [Vikshep Quantum](https://github.com/samvardhan03/vikshep-quantum),
a deterministic quantum state-vector simulator whose results are
bit-identical on every platform and for every thread count. The crate
implements specification
[VQS-1](https://github.com/samvardhan03/vikshep-quantum/blob/main/spec/VQS-1.md):

- Tier-1 complex arithmetic on binary32, with the flush-to-zero rules of
  VDS-1, from `vikshep-numerics`;
- 16 named gates and explicit unitary matrices, with any number of controls;
- circuits with a canonical form and a SHA3 hash;
- Pauli strings;
- the normative reference kernels: gate application, the probability tree,
  sampling and Pauli expectation values;
- the `StateBackend` trait that every backend implements.

```rust
use vikshep_qcore::{Circuit, PauliString, QError};

fn main() -> Result<(), QError> {
    let mut circuit = Circuit::new(2)?;
    circuit.h(0)?.cx(0, 1)?;
    let hash = circuit.hash(); // SHA3-256 of the canonical form, in hex
    let zz = PauliString::parse(2, "Z0 Z1")?;
    println!("{hash} {}", zz.canonical());
    Ok(())
}
```

To run circuits, use the CPU reference backend in
[`vikshep-statevec`](https://crates.io/crates/vikshep-statevec).

Licence: AGPL-3.0-or-later; a commercial licence is available
([LICENSING.md](https://github.com/samvardhan03/vikshep-quantum/blob/main/LICENSING.md)).
