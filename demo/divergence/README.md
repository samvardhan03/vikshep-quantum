# Divergence: do simulators give the same bits when the machine changes?

This demo runs the same circuits with the same seeds on three simulators,
under different settings and on six machines, and checks whether the results
are the same **byte for byte**. It tests the claim behind Vikshep Quantum:
that results from mainstream simulators depend on the machine they run on.

- **Simulators:** Qiskit Aer 0.18.4, PennyLane Lightning 0.45.0
  (`lightning.qubit`) and Vikshep Quantum 0.1.0.
- **Results:** recorded 2026-11-21.
  - [`results/report.md`](results/report.md) covers all six machines.
  - [`results/report-ci.md`](results/report-ci.md) covers the four CI
    machines, which all ran Lightning.
  - `results/*/summary.json` holds the raw data.

## Summary

| | Aer | Lightning | Vikshep Quantum |
|---|---|---|---|
| State bytes across machines | Differ (double: 5 different results on 6 machines) | Differ (double: 4 different results on 5 machines) | **Identical on all 6** |
| Seeded shots and training, double precision | Identical on all 6 | Identical on all 5 | **Identical on all 6** |
| Seeded shots and training, single precision | ARM Linux differs from the rest; **crashed on Windows** | 4 different results on 5 machines | **Identical on all 6** |
| Single-precision error of an energy, against double precision | 9e-6 | 9e-4 on Linux, **0.027 on macOS and Windows** | 1.5e-5 everywhere |

## Machines

| Label | System | CPU | Logical CPUs | Where |
|---|---|---|---:|---|
| `ubuntu-latest` | Linux x86_64 | AMD EPYC 7763 (AVX2, FMA) | 2 | GitHub Actions |
| `ubuntu-24.04-arm` | Linux AArch64 | Arm (ASIMD, SVE, SVE2) | 2 | GitHub Actions |
| `macos-15` | macOS arm64 | Apple M2 Pro (virtual) | 5 | GitHub Actions |
| `windows-latest` | Windows x86_64 | AMD (family 25) | 2 | GitHub Actions |
| `linux-x86_64-docker` | Linux x86_64 (Docker Desktop VM) | Intel Core i7-7700HQ (AVX2, FMA) | 8 | Laptop |
| `macos-x86_64` | macOS 13.7 x86_64 | Intel Core i7-7700HQ | 8 | Laptop |

The two laptop runs share a CPU, so a difference between them comes from the
operating system and the build of the library.

**Where each simulator ran:**

- **Lightning:** not on `macos-x86_64`, because PyPI has no compiled
  Lightning wheel for Intel macOS.
- **The laptop runs** tried every setting listed below.
- **The CI runs** (`--defaults-only`) used the default settings, with all
  threads and with one thread.

## What was run

**Circuits:**

| Name | Qubits | Gates | Content |
|---|---:|---:|---|
| `random16` | 16 | 705 | Random RX/RY/RZ layers with a brick of CX/CZ |
| `random20` | 20 | 472 | The same construction |
| `qft20` | 20 | 230 | Random RY product state, then the quantum Fourier transform |

**For each circuit and setting, the demo records:**

- the state vector;
- the probabilities;
- **1,000,000 measurement shots** with a fixed seed;
- expectation values of `Z0 Z1`, `X3 Y7 Z11`, the parity of all qubits, and
  a transverse-field Ising Hamiltonian.

Each simulator computes these by its own native means.

**Training run.** QAOA for MaxCut on a 16-vertex 3-regular graph (depth 2),
optimised by SPSA. Each step uses two energies estimated from 10,000 seeded
shots, and the run is 150 steps. The optimiser uses only correctly rounded
arithmetic, so any difference between runs comes from the simulator.

**Settings varied:**

| Simulator | Settings | Default |
|---|---|---|
| Aer | thread count × gate fusion (on, off) × precision (double, single) | all threads, fusion on |
| Lightning | thread count; kernel (`LM`, `AVX2`, `AVX512`); precision (complex128, complex64) | all threads, default kernel |
| Vikshep Quantum | thread count (binary32 only) | all threads |

Every setting runs in its own process, so `OMP_NUM_THREADS` takes effect.

## Results

### 1. Across machines, with the default settings

The table groups machines that gave identical bytes. The x86 Linux pair is
`ubuntu-latest` with `linux-x86_64-docker`; "the other four" means the x86
Linux pair, `macos-x86_64` and `macos-15`.

| Simulator, precision | State vector | 1,000,000 seeded shots | Training run |
|---|---|---|---|
| Aer, double | 5 groups: the x86 Linux pair; each other machine alone | Identical on all 6 | Identical on all 6 |
| Aer, single | `ubuntu-24.04-arm` alone, against the other four; crashed on `windows-latest` | The same split | ARM differs from the first step (largest parameter difference 0.12) |
| Lightning, double | 4 groups: the x86 Linux pair; ARM, M2 and Windows each alone | Identical on all 5 | Identical on all 5 |
| Lightning, single | 3 or 4 groups (M2 and Windows agree on `qft20` only) | 4 groups: the x86 Linux pair; ARM, M2 and Windows each alone | Differs between groups from the first step (largest parameter difference 0.027) |
| Vikshep Quantum | **Identical on all 6** | **Identical on all 6** | **Identical on all 6** |

The groups are the same for all three circuits, except where noted.
Expectation values spread across machines as follows:

| Simulator, precision | Largest spread |
|---|---:|
| Aer, double | 2.0e-13 |
| Lightning, double | 3.1e-13 |
| Aer, single | 3.2e-7 |
| Lightning, single | 2.6e-2 |
| Vikshep Quantum | 0 |

On the laptop, 99.6 to 99.9% of Aer's double-precision amplitudes differ
between macOS and Linux, each by at most 2.7e-17 (a few units in the last
place).

### 2. On one machine, when a setting changes

| What changed | Aer | Lightning | Vikshep Quantum |
|---|---|---|---|
| Thread count | Expectation values differ (up to 2.3e-13 double). States are identical on x86. On ARM Linux the single-precision state differs too (by 9.3e-10), and 26 of 1,000,000 shots change. | Expectation values differ (up to 3.6e-15 double, 1.9e-6 single); states identical | Identical bytes |
| Kernel (`LM`, `AVX2`, `AVX512`) | — | Expectation values differ (up to 1.8e-15 double, 9.5e-7 single); states identical | — |
| Gate fusion off, single precision | Up to 27% of the shots change (below); training differs from the first step | — | — |

Lightning accepted `kernel_name="AVX512"` on the laptop, whose CPU has no
AVX-512. We did not check what it ran instead.

### 3. Double precision: the shots and the training run held

In every double-precision comparison, the 1,000,000 seeded shots and the
QAOA training run were identical, step for step. This held across 6 machines
for Aer and 5 for Lightning, and across every setting on one machine. The
differences stay in the last bits of states and expectation values.

### 4. Single precision: a tiny change in the state changes the shots

On the laptop, turning off Aer's gate fusion changes the single-precision
probabilities by at most 2.6e-9. Yet many of the 1,000,000 seeded shots
change:

| Circuit | Shots that changed (of 1,000,000) |
|---|---:|
| `random16` | 22,268 |
| `qft20` | 95,694 |
| `random20` | 271,640 (27%) |

The same happens across machines, as section 1 shows:

- **Aer:** ARM Linux disagrees with the other four machines.
- **Lightning:** the five machines give four different shot records.

Where the shots differ, the training run differs from its first step. The
final sampled cuts stay within about 0.02 of each other (about 17.7 of 24
edges), so the conclusion is the same but the numbers are not.

A likely reason, not checked in either library's source: a shot is assigned
by comparing a random number with running sums of 2^20 probabilities. In
single precision, every one of those sums moves when the state changes
slightly, and the total of the moves is a large fraction of the probability
mass.

### 5. Single-precision accuracy depends on the platform for Lightning

The Ising energy of `qft20`, against the double-precision value 11.1132:

| Simulator | Platform | Error |
|---|---|---:|
| Lightning, single | macOS (M2) and Windows | **−0.027** |
| Lightning, single | Linux (x86 and ARM) | −9.2e-4 |
| Aer, single | — | −8.9e-6 |
| Vikshep Quantum (binary32, pairwise sums) | every platform | −1.5e-5 |

This is a difference in accuracy, not only in the last bits: the same
program is about 30 times less accurate on two of the platforms.

### 6. Aer crashed on Windows in single precision

On `windows-latest`, both single-precision Aer runs (2 threads and 1 thread)
ended with an access violation (exit code 3221225477, 0xC0000005) in Aer's
native code. The crash came during the first evaluation: a 16-qubit circuit
saving the state vector, the probabilities and four expectation values. The
double-precision runs completed.

We narrowed it down afterwards: one RZ gate on a 2-qubit register is
enough. In single precision on Windows x86_64, the 12-line reproducer
crashed in 20 of 20 runs with Aer 0.18.4 and with 0.17.2, with an access
violation or heap corruption (0xC0000374). Double precision never crashed,
and on Linux it never crashed. It is reported as
[Qiskit/qiskit-aer#2541](https://github.com/Qiskit/qiskit-aer/issues/2541).

### 7. Vikshep Quantum: the same bytes everywhere

On all six machines and at every thread count, the following were identical
byte for byte:

- states;
- probabilities;
- all 1,000,000 shots;
- every expectation value;
- the training run.

On every machine, the simulators agree with each other on the probabilities
to within 7.3e-9 in single precision and 4.8e-17 in double precision. This
checks that the three adapters build the same circuits.

## What this means

- **The bytes change, at every precision.** Mainstream simulators' states
  and expectation values differ between machines, and between thread counts
  and kernels on one machine. Anything that compares results exactly breaks
  across machines:
  - golden-file tests;
  - content hashes and caches;
  - verification by recomputation;
  - dataset identity.
- **In double precision, the science held.** Seeded shots and shot-based
  training were identical on every machine tested. For double precision on
  CPUs, the honest pitch is "your bytes change".
- **In single precision, the shots, the training runs and even the accuracy
  change:**
  - between machines (ARM against x86 for Aer; four results on five
    machines for Lightning);
  - with settings, such as fusion;
  - with the platform's accuracy (a 30-fold difference for Lightning).

  Single precision is the usual choice on GPUs, for speed and memory. GPUs
  were not tested here.
- **Vikshep Quantum's guarantee held.** It gave identical bytes on six
  machines (Linux, macOS and Windows; x86_64 and ARM) and at every thread
  count. Its binary32 accuracy was in line with Aer's single precision.

## Limits

- **Virtual machines.** The CI machines are virtual, with 2 to 5 vCPUs.
- **One version of each library.**
- **No GPU.**
- **Small scope.** Circuits have up to 20 qubits, and there is one training
  problem with one optimiser.
- **Gaps in coverage:**
  - Lightning did not run on Intel macOS.
  - Aer single precision did not run on Windows.
  - The CI machines ran only the default settings.
- **Aer on macOS** is built without OpenMP. Its thread setting still changed
  the expectation values.
- **Unchecked explanations.** The explanations in sections 4 and 5 are not
  checked against the libraries' source code.

## Reproducing

```bash
python -m venv .venv && . .venv/bin/activate
pip install -r demo/divergence/requirements.txt
maturin build --release --locked -m crates/vikshep-quantum-py/Cargo.toml -o dist
pip install --no-index --find-links dist vikshep-quantum
python -I demo/divergence/divergence.py run --label mymachine --out out/mymachine
python -I demo/divergence/divergence.py report out/mymachine demo/divergence/results/*/
```

**Run times:** 21 minutes on the laptop under macOS, 27 minutes in its Linux
VM, and 3 to 7 minutes per CI runner with `--defaults-only`, including the
wheel build.

**Options:**

- `--quick` runs a smoke test.
- `--defaults-only` runs only the default settings, with all threads and
  with one thread.
- `--skip-train` leaves out the training run.

A crashed setting is recorded in the summary with its exit code and a
traceback, and the run continues.

**In CI:** run [`.github/workflows/divergence.yml`](../../.github/workflows/divergence.yml)
by hand. It takes a JSON list of platforms. The CI results here came from
two runs of this workflow before the repository was published: one on Linux
x86_64 and AArch64 and macOS arm64, and one on Windows after a fix that
records crashes instead of stopping.

**In Docker on a Mac:** cross-compile the wheel with `maturin build --zig
--target x86_64-unknown-linux-gnu` and mount it into a `python:3.11-slim`
container.
