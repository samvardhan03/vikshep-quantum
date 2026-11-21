# vikshep-statevec

The CPU reference backend of
[Vikshep Quantum](https://github.com/samvardhan03/vikshep-quantum), a
deterministic quantum state-vector simulator. Amplitudes, probabilities,
samples and expectation values are bit-identical on every platform and for
every thread count, as specification
[VQS-1](https://github.com/samvardhan03/vikshep-quantum/blob/main/spec/VQS-1.md)
requires. Every run can produce a canonical manifest whose hash identifies
exactly what was computed.

```rust
use vikshep_qcore::{Circuit, QError};
use vikshep_statevec::Simulator;

fn main() -> Result<(), QError> {
    let mut circuit = Circuit::new(2)?;
    circuit.h(0)?.cx(0, 1)?;
    let mut sim = Simulator::new(2)?;
    sim.run(&circuit)?;
    let shots = sim.sample(7, 0, 100)?; // seed, stream, shots
    let zz = sim.expectation_str("Z0 Z1")?;
    let manifest = sim.manifest()?; // canonical JSON run record
    println!("{} shots, <Z0 Z1> = {zz}\n{manifest}", shots.len());
    Ok(())
}
```

The backend is multi-threaded. The work is divided so that every amplitude
is computed by the same operations in the same order, whichever thread
computes it, so the thread count never changes a result.

Licence: AGPL-3.0-or-later; a commercial licence is available
([LICENSING.md](https://github.com/samvardhan03/vikshep-quantum/blob/main/LICENSING.md)).
