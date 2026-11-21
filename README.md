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

> **Status: 0.1.0.** This release has:
>
> - the CPU reference;
> - the specification;
> - conformance suite v1;
> - the Python package.
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
| Correct results | Agreement with independent binary64 simulators (Rust and numpy), the closed-form QFT, dense Pauli operators, sample statistics, and Qiskit 2.6.2 `Statevector` |
| The suite detects non-conforming arithmetic | An independent kernel with the specified arithmetic passes all 257 cases; the same kernel without the flush, with after-rounding tininess, or skipping zero matrix entries fails 27, 11 and 4 cases |

---

## Layout

| Path | What it is | Licence |
|---|---|---|
| `crates/vikshep-qcore` | Tier-1 complex arithmetic, gate matrices, circuits, Pauli strings, the normative kernels and the backend trait | AGPL-3.0 |
| `crates/vikshep-statevec` | CPU reference backend (multi-threaded, bit-identical) and the `Simulator` | AGPL-3.0 |
| `crates/vikshep-qconformance` | Conformance suite v1 (257 cases), runner and CLI | AGPL-3.0 |
| `crates/vikshep-quantum-py` | Python wheel `vikshep-quantum` (abi3, Python 3.10+) | AGPL-3.0 |
| `spec/VQS-1.md` | The specification | CC-BY-4.0 |
| `conformance/vectors/v1` | Expected output hashes | CC-BY-4.0 |
| `scripts/determinism-lint.sh` | Rejects platform-libm and fused float calls | AGPL-3.0 |
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
