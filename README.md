# Vikshep Quantum

**Deterministic quantum state-vector simulation. The same circuit gives the
same bits on every machine.**

Vikshep Quantum simulates quantum circuits on a state vector and guarantees
that amplitudes, probabilities, samples and expectation values are
bit-identical across operating systems, CPU architectures, thread counts
and, as they are added, GPU backends. Every run can produce a manifest whose
hash identifies exactly what was computed. The arithmetic is the Vikshep
open core's ([`vikshep-compute`](https://github.com/samvardhan03/vikshep-compute),
specification VDS-1); this repository adds the quantum layer, specified in
[`spec/VQS-1.md`](spec/VQS-1.md).

> **Status: 0.2.0.** This release has:
>
> - the CPU reference;
> - the specification;
> - conformance suite v1;
> - the Python package, with a [Qiskit backend and primitives](#use-it-from-qiskit)
>   (new in 0.2.0).
>
> Suite v1 gives byte-identical reports on Linux x86_64 and AArch64, macOS
> arm64 and x86_64, and Windows x86_64. GPU backends are not released yet.
> See [STATUS.md](STATUS.md).

```bash
pip install vikshep-quantum
```

---

## Why determinism

Floating-point results usually depend on the machine. Thread scheduling,
fused multiply-add, vendor math libraries and subnormal handling all change
the last bits, and in sampled experiments the last bits can change the
answer. That makes simulated results hard to reproduce, compare across
teams, or audit.

We measured this ([`demo/divergence`](demo/divergence/README.md)): the same
circuits with the same seeds on six machines covering Linux, macOS and
Windows, x86_64 and ARM.

| | Qiskit Aer 0.18.4 | PennyLane Lightning 0.45.0 | Vikshep Quantum |
|---|---|---|---|
| State bytes across machines | Differ | Differ | **Identical on all 6** |
| 1,000,000 seeded shots and a shot-based training run, double precision | Identical | Identical | **Identical** |
| The same, single precision | Differ (ARM Linux against the rest) | Differ (4 results on 5 machines) | **Identical** |

In double precision the differences stayed in the last bits. In single
precision, the usual choice on GPUs, they changed the shots and the
training runs.

VQS-1 fixes every operation, its order and its rounding, so a result is a
function of its inputs alone. Uses:

- **Reproducible research.** A quantum machine-learning or algorithm result
  can be re-run anywhere and checked byte for byte, not "within tolerance".
- **Reference results.** Hardware, compiler and error-correction teams get a
  replayable reference with a provenance record, including the exact shots.
- **Simulated training data.** Datasets of states or samples (for example
  for decoder training) are identified by hashes and regenerated exactly from
  their seeds.

---

## Quickstart (Python)

```bash
pip install vikshep-quantum
```

PyPI has wheels for Linux (x86_64, AArch64), macOS (arm64, x86_64) and
Windows (x86_64), for Python 3.10 and later. On other platforms, build from
this repository (needs Rust 1.97 and [maturin](https://www.maturin.rs)):

```bash
pip install maturin numpy
maturin build --release -m crates/vikshep-quantum-py/Cargo.toml -o dist
pip install dist/vikshep_quantum-*.whl
```

```python
import vikshep_quantum as vq

bell = vq.Circuit(2).h(0).cx(0, 1)
sim = vq.Simulator(2)
sim.run(bell)

samples = sim.sample(1000, seed=7)        # uint64, the same on every machine
print(vq.counts(samples, 2))              # {'00': ..., '11': ...}
print(sim.expectation("Z0 Z1"))           # 1.0 up to binary32 rounding
print(sim.state_oid())                    # 28-hex OID of the state bytes
print(sim.manifest())                     # canonical JSON run record
print(vq.conformance_selftest()["failed"])  # 0
```

Qubit 0 is the least significant bit of a basis-state index, as in Qiskit.
See [`examples/quickstart.py`](examples/quickstart.py).

## Use it from Qiskit

`vikshep_quantum.qiskit` has a Qiskit backend and the V2 primitives
(Qiskit 2.0 or later), from version 0.2.0. Install it with the `qiskit`
extra:

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

result = VikshepSimulator().run(measured, shots=1000, seed_simulator=7).result()
result.get_counts()                                   # the same on every machine
result.results[0].metadata["manifest"]                # run record of the final state

Sampler(seed=7).run([measured], shots=1000).result()[0].data.meas.get_counts()
Estimator().run([(bell, "ZZ")]).result()[0].data.evs  # exact, the same bits everywhere
```

- **No transpiling needed.** A gate with a VQS-1 equivalent (`h`, `rz`,
  `u`, `cx`, `cp`, `crx`, `ccx`, `mcx`, `unitary`, ...) becomes one VQS-1
  operation with the same parameters. Every other gate (`rzz`, `ecr`,
  `QFTGate`, `PauliEvolutionGate`, ...) is replaced by its Qiskit
  definition, which passes the angles through. The conversion computes no
  sine, cosine or matrix product of its own. `from_qiskit(circuit)` returns
  the converted `vikshep_quantum.Circuit`.
- **Seeds and streams.** Circuit `i` of a `run` call draws its shots from
  the Philox stream `(seed_simulator, i)`, so
  `vikshep_quantum.Simulator.sample(shots, seed, i)` reproduces them. The
  `Sampler` numbers the circuits of a call across pubs and parameter
  bindings. Without a seed, a random one is used and reported in the result.
- **Exact estimator.** An observable `sum_k c_k P_k` is evaluated as
  `sum_k c_k <P_k>`. Each `<P_k>` is a VQS-1 binary32 expectation value,
  and the sum is taken in binary64 in sorted label order. The precision
  must be 0; use the `Sampler` for shot-based estimates.
- **The same inputs give the same bits.** Angles drawn with numpy's
  `Generator.uniform(-pi, pi)` had different last bits on macOS arm64 in
  our CI, which makes a different circuit. If the parameters must match
  across machines, build them from integers, as
  [`scripts/qiskit-record.py`](scripts/qiskit-record.py) does.
- **Not supported:** mid-circuit measurement, a reset after a gate,
  classical control flow and noise models.
- **If you transpile, use `optimization_level=0`.** The results of a given
  circuit are bit-identical; the circuit the transpiler returns may not be.
  With Qiskit 2.6.2, in our CI:
  - Level 0 returned the same circuit on all four platforms.
  - Level 1 returned the same circuit on each machine, but a different one
    on macOS arm64, Windows and Linux.
  - Levels 2 and 3 (2 is the default of `transpile`) returned a different
    circuit on every call for targets with many gates, even with a fixed
    `seed_transpiler`. This happened with this backend, with Qiskit Aer
    and with Qiskit's `BasicSimulator`.

See [`examples/qiskit_quickstart.py`](examples/qiskit_quickstart.py).

## Quickstart (Rust)

```rust
use vikshep_qcore::Circuit;
use vikshep_statevec::Simulator;

let mut circuit = Circuit::new(2)?;
circuit.h(0)?.cx(0, 1)?;
let mut sim = Simulator::new(2)?;
sim.run(&circuit)?;
let shots = sim.sample(7, 0, 100)?;        // seed, stream, shots
let zz = sim.expectation_str("Z0 Z1")?;
let manifest = sim.manifest()?;
```

---

## What is guaranteed, and what has been checked

| Claim | Evidence today |
|---|---|
| Same bytes for any thread count | Rust and Python tests compare 1, 2, 3, 5 and 8 threads on 18-qubit circuits |
| Same bytes on every platform | Suite v1 reports are all-pass and byte-identical on Linux x86_64 and AArch64, macOS arm64 and Windows x86_64 (the `hash-diff` job of [CI](.github/workflows/ci.yml), on every pull request) and on macOS x86_64 (locally) |
| Same Qiskit results on every platform | The `qiskit-diff` job of [CI](.github/workflows/ci.yml) compares records of seeded backend runs, `Sampler` shots and `Estimator` values from the same four platforms, on every pull request |
| Correct results | Agreement with independent binary64 simulators (Rust and numpy), the closed-form QFT, dense Pauli operators, sample statistics, and Qiskit 2.6.2 `Statevector`. The Qiskit integration is tested against Qiskit's `Statevector`, `BasicSimulator`, `StatevectorSampler` and `StatevectorEstimator`, with Qiskit 2.0.0 and 2.6.2 |
| The suite detects non-conforming arithmetic | An independent kernel with the specified arithmetic passes all 257 cases; the same kernel without the flush, with after-rounding tininess, or skipping zero matrix entries fails 27, 11 and 4 cases |

---

## Layout

| Path | What it is | Licence |
|---|---|---|
| `crates/vikshep-qcore` | Tier-1 complex arithmetic, gate matrices, circuits, Pauli strings, the normative kernels and the backend trait | AGPL-3.0 |
| `crates/vikshep-statevec` | CPU reference backend (multi-threaded, bit-identical) and the `Simulator` | AGPL-3.0 |
| `crates/vikshep-qconformance` | Conformance suite v1 (257 cases), runner and CLI | AGPL-3.0 |
| `crates/vikshep-quantum-py` | Python wheel `vikshep-quantum` (abi3, Python 3.10+), with the Qiskit backend and primitives in `python/vikshep_quantum/qiskit` | AGPL-3.0 |
| `spec/VQS-1.md` | The specification | CC-BY-4.0 |
| `conformance/vectors/v1` | Expected output hashes | CC-BY-4.0 |
| `scripts/determinism-lint.sh` | Rejects platform-libm and fused float calls | AGPL-3.0 |
| `scripts/qiskit-record.py` | Seeded Qiskit workloads that CI records and compares across platforms | AGPL-3.0 |
| `demo/divergence` | The cross-machine comparison with Qiskit Aer and PennyLane Lightning, and its results | AGPL-3.0 |

## Conformance

```bash
cargo run --release -p vikshep-qconformance -- run              # prints the report
cargo run --release -p vikshep-qconformance -- run --threads 1   # same bytes
cargo run --release -p vikshep-qconformance -- list              # case ids
```

A backend conforms when it reproduces every case; partial conformance is
non-conformance (VQS-1 section 11).

## Open core

The specification, the conformance vectors, the CPU reference and the
Python package are open. Faster backends (Metal, then CUDA) are planned as
proprietary binaries that must pass the same open suite, following the
model of `vikshep-compute` and `vikshep-compute-pro`. See
[LICENSING.md](LICENSING.md).

## Limits

- binary32 amplitudes only; accuracy decreases with circuit depth (VQS-1
  section 14 records measured errors).
- Up to 40 qubits by specification; in practice, the state must fit in
  memory (`2^n * 8` bytes: 26 qubits is 512 MiB).
- The CPU reference favours exactness over speed. On an Intel Core
  i7-7700HQ a 24-qubit gate takes about 73 ms with 8 threads (STATUS.md).
- No mid-circuit measurement, noise, gradients or tensor networks yet
  (VQS-1 section 13 lists them as open decisions).

## Licence

AGPL-3.0-or-later for code, CC-BY-4.0 for the specification and vectors; a
commercial licence is available. See [LICENSING.md](LICENSING.md).
