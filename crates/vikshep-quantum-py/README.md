# vikshep-quantum

A deterministic quantum state-vector simulator. States, probabilities,
seeded measurement samples and expectation values have the same bits on
every supported platform and for every thread count. Every run can produce
a manifest whose hash identifies exactly what was computed
([specification VQS-1](https://github.com/samvardhan03/vikshep-quantum/blob/main/spec/VQS-1.md)).

```bash
pip install vikshep-quantum
```

Wheels are built for Linux (x86_64, AArch64), macOS (arm64, x86_64) and
Windows (x86_64), for Python 3.10 and later. The distribution is called
`vikshep-quantum`, and the import name is `vikshep_quantum`.

```python
import vikshep_quantum as vq

bell = vq.Circuit(2).h(0).cx(0, 1)
sim = vq.Simulator(2)
sim.run(bell)
print(vq.counts(sim.sample(1000, seed=7), 2))   # {'00': ..., '11': ...}
print(sim.expectation("Z0 Z1"))                 # 1.0 up to binary32 rounding
print(sim.state_oid())                          # 28-hex OID of the state bytes
print(sim.manifest())                           # canonical JSON run record
print(vq.conformance_selftest()["failed"])      # 0
```

Qubit 0 is the least significant bit of a basis-state index, as in Qiskit.
The state is complex64 (binary32 components). Arrays passed in are
converted with numpy and copied in row-major order before any computation,
so strides, memory order and byte order never change a result.

## From Qiskit

```bash
pip install "vikshep-quantum[qiskit]"
```

```python
from qiskit import QuantumCircuit
from vikshep_quantum.qiskit import Estimator, Sampler, VikshepSimulator

bell = QuantumCircuit(2)
bell.h(0)
bell.cx(0, 1)
measured = bell.measure_all(inplace=False)

VikshepSimulator().run(measured, shots=1000, seed_simulator=7).result().get_counts()
Sampler(seed=7).run([measured], shots=1000).result()[0].data.meas.get_counts()
Estimator().run([(bell, "ZZ")]).result()[0].data.evs
```

Circuits need not be transpiled: gates are converted directly or expanded
with their Qiskit definitions. Measurements must come at the end of the
circuit, and the estimator is exact. If you do transpile, use
`optimization_level=0`: at higher levels Qiskit can return a different
circuit on another machine, or on each call. Details are in the
[README](https://github.com/samvardhan03/vikshep-quantum#use-it-from-qiskit).

## Why it matters

In a measured comparison on six machines, Qiskit Aer and
PennyLane Lightning gave different state bytes on different machines. In
single precision, they also gave different seeded shots and different
training runs. Vikshep Quantum gave the same bytes on all six
([demo/divergence](https://github.com/samvardhan03/vikshep-quantum/tree/main/demo/divergence)).

Licence: AGPL-3.0-or-later; a commercial licence is available
([LICENSING.md](https://github.com/samvardhan03/vikshep-quantum/blob/main/LICENSING.md)).
