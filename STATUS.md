# Status

Current release: **0.1.0**. It contains:

- the CPU reference;
- the specification VQS-1;
- conformance suite v1;
- the Python package `vikshep-quantum`.

In progress: integrations (Q4). The Qiskit backend and primitives are on
`main` and not released yet; the PennyLane device is next. GPU backends
come after.

## Q4: Qiskit integration (on `main`, not released)

`vikshep_quantum.qiskit` (extra `qiskit`, Qiskit 2.0 or later):

- `VikshepSimulator`, a `BackendV2`: counts, per-shot memory and a run
  manifest per circuit;
- `Sampler` and `Estimator`, the V2 primitives; the estimator is exact;
- `from_qiskit`, which converts a Qiskit circuit into a
  `vikshep_quantum.Circuit`.

Gates with a VQS-1 equivalent become one operation with the same
parameters. Other gates are expanded with their Qiskit definitions, so the
conversion computes no trigonometry of its own.

Checked:

- `tests/test_qiskit_backend.py` passes with Qiskit 2.6.2 (Python 3.13)
  and Qiskit 2.0.0 (Python 3.11): 120 tests.
  - Every standard gate, alone and with two open or closed controls, gives
    Qiskit's `Statevector` up to global phase.
  - So do QFT, Pauli evolution, `efficient_su2`, random circuits,
    `initialize` and circuits transpiled at levels 0 to 3.
  - Counts equal `BasicSimulator`'s on deterministic circuits.
  - The `Sampler`'s bit layout equals `StatevectorSampler`'s.
  - `Estimator` values agree with `StatevectorEstimator` to 1e-5.
  - Backend memory, `Sampler` shots and `Estimator` values equal the
    native API bit for bit, for 1 and 4 threads.
- The CI job `qiskit-diff` compares records of seeded workloads
  (`scripts/qiskit-record.py`) from Linux x86_64 and AArch64, macOS arm64
  and Windows x86_64, on every pull request.

Found on the way: with Qiskit 2.6.2, `transpile` at optimization levels 2
and 3 returned a different circuit on every call, even with a fixed
`seed_transpiler`. This happened for targets with many gates: this
backend's, Qiskit Aer's and Qiskit's `BasicSimulator`'s. With a basis of
`u` and `cx`, or `rz`, `sx`, `x` and `cx`, it was stable, and level 1 was
stable for every target. The README advises not transpiling, or using
level 0 or 1. Not yet reported to Qiskit.

Also found: in the first CI run of `qiskit-diff`, macOS arm64 differed from
the other three platforms only in the run records of circuits whose angles
came from numpy's `Generator.uniform(-pi, pi)`. Counts, shots and
`Estimator` values were identical. A fused multiply-add in numpy's
`low + (high - low) * u` explains it: fusing changes 70 of the 128 angles of
one of those circuits, and none when `low` is 0, as for the circuits that
matched. The record now builds its angles from integers, and the
information-only part of the record keeps numpy's draws.

## Q1: cross-platform CI (complete)

The conformance reports from these four platforms are all-pass and
byte-identical:

- Linux x86_64 (`ubuntu-latest`);
- Linux AArch64 (`ubuntu-24.04-arm`);
- macOS arm64 (`macos-15`);
- Windows x86_64 (`windows-latest`).

Their SHA-256 is
`53f352339ee1d98180dd0045ccff5c1470e7efe97f2eefbe0ae2d8ae8969fd3d`, the same
as the local report on macOS x86_64, so suite v1 gives the same bytes on five
platforms. On each platform, the one-thread report equals the default one,
and the wheel passes pytest (19 passed; the Qiskit tests are skipped because
CI does not install Qiskit).

These runs were made before the repository was published. The `hash-diff`
job of [CI](.github/workflows/ci.yml) repeats the comparison on every pull
request.

## Divergence demo (complete)

[`demo/divergence`](demo/divergence/README.md) compares Qiskit Aer,
PennyLane Lightning and this package on six machines. Only this package gave
the same bytes on all of them.

## Q0 checklist

- [x] Workspace: Rust 1.97.0, edition 2024, exact dependency pins,
      `unsafe_code = "forbid"` outside the Python bindings, Clippy
      configuration and determinism lint from `vikshep-compute`
- [x] `vikshep-qcore`: Tier-1 complex arithmetic on the VDS-1.2 flush,
      16 named gates plus explicit matrices with any number of controls,
      circuits with canonical form and hash, Pauli strings, normative kernels
      (gate application, probability tree, sampling, expectation values),
      `StateBackend` trait
- [x] `vikshep-statevec`: CPU reference backend, multi-threaded with results
      independent of the thread count; `Simulator` with state OIDs and run
      manifests
- [x] `vikshep-qconformance`: suite `vqs1-v1`, 257 cases in 8 families,
      runner, CLI, vectors compiled into the library
- [x] `vikshep-quantum-py`: abi3 wheel for Python 3.10+, `Circuit`,
      `Simulator`, `counts`, `conformance_selftest`, `oid`
- [x] `spec/VQS-1.md` sections 1 to 14
- [x] CI workflow (`.github/workflows/ci.yml`) on every push to `main` and
      every pull request; the same checks run locally with
      `bash scripts/ci-local.sh --python`
- [x] CI run on Linux x86_64, Linux AArch64, macOS arm64, Windows x86_64
      with byte-identical reports (Q1)

## Verified locally (macOS 13.7, Intel Core i7-7700HQ, 4 cores / 8 threads, 16 GB)

| Check | Result |
|---|---|
| `cargo test --workspace` | 45 tests pass |
| `cargo clippy` (workspace, and the bindings with `--features python`), `-D warnings` | clean |
| `cargo fmt --check`, `scripts/determinism-lint.sh` | clean |
| Conformance suite v1, CPU reference | 257 / 257, identical reports with 1 and 8 threads (report SHA-256 `53f352339ee1d98180dd0045ccff5c1470e7efe97f2eefbe0ae2d8ae8969fd3d`) |
| Independent kernel, specified arithmetic | 257 / 257 |
| Same kernel, no flush / after-rounding tininess / skipped zero entries | 27 / 11 / 4 cases fail, as they must |
| Largest amplitude error against binary64 oracle (42 random circuits, 1 to 7 qubits, 80 operations) | 3.92e-7 |
| Python tests (abi3 wheel on Python 3.11) | 25 pass with Qiskit installed; without it the 6 Qiskit tests are skipped and 19 pass |
| Qiskit 2.6.2 cross-check | 6 pass (15 random circuits of 1 to 8 qubits, and a 6-qubit QFT) |
| Sample statistics (5-qubit GHZ, 200 seeds of 1000 shots) | mean 500.1 and standard deviation 15.83 of the count of 11111, against 500 and 15.81 for a fair binomial |

## Throughput of the CPU reference (same machine)

A layer of H on every qubit, a chain of CX, and RZ on every qubit:

| Qubits | Operations | 1 thread | 8 threads |
|---:|---:|---:|---:|
| 20 | 59 | 0.96 s (16 ms per gate) | 0.33 s (5.6 ms per gate) |
| 24 | 71 | 18.0 s (254 ms per gate) | 5.19 s (73 ms per gate) |
| 26 | 77 | 78.4 s (1.02 s per gate) | 22.2 s (288 ms per gate) |

The final state had the same OID with 1 and 8 threads in every row. The
reference evaluates the flush in software on every operation and runs
controlled gates through the general kernel, so it is several times slower
than throughput-oriented simulators. Speed is the job of the backends (Q2,
Q3) and of specialised CPU kernels that must reproduce the same bytes.

## Known limitations

- Computing a state OID or a manifest copies the state twice (the trait
  returns the state by value, then it is serialized); at 28 qubits and above
  that is significant memory.
- Controlled single-target gates (CX, CZ, CP, ...) use the general kernel;
  a specialised kernel would be faster with identical bytes.
- Sampling resolves each split to 2^-24 of the node's mass (VQS-1 section 8).
- Manifests do not record samples or expectation values (D-Q-REC).
- The determinism lint was adapted for macOS (bash 3.2, BWK awk): no
  `mapfile`, and the pattern is POSIX ERE passed through the environment.
  The `vikshep-compute` copy still uses `mapfile` and the GNU-style pattern.

## Next milestones

- **Q4: integrations.** Qiskit backend and primitives (on `main`), then the
  PennyLane device and OpenQASM 3 import.
- **Q2 and Q3: GPU backends**, proprietary, following the model of
  `vikshep-compute-pro`. Each must pass the open suite v1 on its GPU.
  - **Q2, Metal:** the kernel of VQS-1 section 5 in MSL with the settings
    of VDS-1 section 3.3.
  - **Q3, CUDA:** the same kernel on NVIDIA GPUs.
- **Q5: gradients** (adjoint method; decision D-Q-GRAD) for quantum machine
  learning.
