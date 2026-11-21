# VQS-1: Vikshep Quantum Simulation Specification, version 1

| | |
|---|---|
| Status | Draft. Complete for `vqs_version = 1` on `numerics_version = 3` (state-vector simulation: gate application, probabilities, sampling, Pauli expectation values, circuit and run records) |
| VQS version | 1 |
| Numerics version | 3 (VDS-1.2, inherited) |
| Reference implementation | this repository (`vikshep-quantum`), Rust CPU code |
| Copyright | Samvardhan Singh |
| Licence | CC-BY-4.0 (see `LICENSING.md`) |

The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT,
RECOMMENDED, MAY and OPTIONAL are to be interpreted as described in BCP 14
(RFC 2119, RFC 8174) when, and only when, they appear in all capitals.

Notation follows VDS-1: `fl32(e)` and `fl64(e)` are the IEEE-754 binary32
and binary64 roundings (round to nearest, ties to even) of the exact value
`e`; `ftz` is the flush of VDS-1 section 8.1; hex values carry a `0x` prefix.

---

## 1. Purpose and scope

VQS-1 defines a quantum state-vector simulator whose results are the same
bytes on every conforming machine: the same circuit, initial state and seed
give the same amplitudes, probabilities, samples and expectation values on
the CPU reference, on any conforming GPU backend, on any operating system and
for any thread count. It defines which computations that covers, the exact
expressions they evaluate, and how a backend proves conformance.

VQS-1 builds on the Vikshep Determinism Specification, VDS-1
(`samvardhan03/vikshep-compute`, `spec/VDS-1.md`, `numerics_version = 3`).
Everything VDS-1 defines and VQS-1 uses is incorporated by reference
(section 2). This specification adds only what is particular to quantum
simulation.

### 1.1 Tiers

The tiers are those of VDS-1 section 1.1.

**Tier 1: bit-exact on every conforming backend.** Gate application
(section 5), probability leaves and the probability tree (section 7), the
multiplications and comparisons of sampling (section 8), and expectation
values (section 9).

**Tier 2: bit-exact because it always runs on the host, in the Rust code of
this repository.** Gate-matrix construction (section 6), circuit validation
and canonical form (section 10.1), hashes, object identifiers and manifests
(sections 10.2 and 10.3), and the conformance runner. A backend MUST NOT
recompute a gate matrix; it receives matrices as binary32 bytes from the
host.

Results from software this specification does not control (other
simulators, compilers that rewrite circuits) are outside its scope.

---

## 2. Inherited definitions

The following parts of VDS-1 at `numerics_version = 3` are normative for
VQS-1 without change:

- section 2, arithmetic model: binary32 Tier-1 values; round to nearest,
  ties to even; each operation rounded individually and evaluated exactly in
  the order written; no reassociation, contraction or fused multiply-add; no
  division and no transcendental functions in Tier-1 kernels; no
  floating-point atomics; no reduction whose order depends on scheduling;
- section 3, backend compile settings (CPU, CUDA, Metal);
- section 4, portable transcendental functions (`vikshep-detmath`);
- section 5, random numbers (SplitMix64, Philox4x32-10, the stream mapping
  and `next_f32_unit`);
- section 8, subnormals: the flush of every binary32 Tier-1 input and result
  (VDS-1.1), with tininess of multiplications judged on the exact product
  before rounding (VDS-1.2);
- section 9, canonical serialization, object identifiers (OIDs) and RFC 8785
  canonical JSON.

In this document `add(a, b)`, `sub(a, b)` and `mul(a, b)` denote the Tier-1
binary32 addition, subtraction and multiplication of VDS-1 section 8:

```
add(a, b) = ftz(fl32(a + b))
sub(a, b) = ftz(fl32(a - b))
mul(a, b) = the VDS-1.2 multiplication (tininess before rounding)
```

Negation is exact (it changes the sign bit only) and is not flushed again.

The reference implementation takes these definitions from the published
crates `vikshep-numerics 0.1.0` and `vikshep-detmath 0.1.0`, pinned exactly.

---

## 3. States

### 3.1 Size

A state of `n` qubits, `1 <= n <= 40`, is `2^n` complex amplitudes, each a
pair of binary32 values (real part, imaginary part). Bit `q` of an amplitude
index `i` is the value of qubit `q`; qubit 0 is the least significant bit.
This is the order Qiskit uses.

### 3.2 Layout

The canonical layout is the `2 * 2^n` binary32 values in index order, the
real part of each amplitude before its imaginary part, little-endian. It is
identical to a numpy `complex64` array of `2^n` elements with little-endian
byte order.

### 3.3 Initial state

The initial state `|0...0>` has amplitude 0 equal to `(1, +0)` and every
other amplitude `(+0, +0)`.

### 3.4 Loaded states

A caller MAY supply a state instead. Every component MUST be finite. The
backend flushes every component (`ftz`) and does not normalize: VQS-1
nowhere divides by a norm, so an unnormalized state stays unnormalized and
every derived quantity scales accordingly.

---

## 4. Complex arithmetic

### 4.1 Values

A complex value is a pair `(re, im)` of binary32 values.

### 4.2 Operations

```
cmul(a, b).re = sub(mul(a.re, b.re), mul(a.im, b.im))
cmul(a, b).im = add(mul(a.re, b.im), mul(a.im, b.re))
cadd(a, b)    = (add(a.re, b.re), add(a.im, b.im))
```

The operands are evaluated left before right as written. No other complex
operation is used in Tier 1.

---

## 5. Gate application

### 5.1 Operations

An operation is a gate matrix `M` on `k >= 1` distinct target qubits
`targets[0..k]` with `c >= 0` control qubits, all distinct from each other
and from the targets. `M` has `2^k x 2^k` entries; bit `m` of a row or
column index is the value of `targets[m]`, so the first target is the least
significant matrix-index bit. The operation acts where every control qubit
is 1 and leaves every other amplitude unchanged, bit for bit.

### 5.2 Definition (normative)

Let `spread(v)` be the index whose bit `targets[m]` is bit `m` of `v` and
whose other bits are 0. For every index `b` whose target bits are all 0 and
whose control bits are all 1:

```
x[col] = amplitude(b | spread(col))                  col = 0 .. 2^k - 1
for row = 0 .. 2^k - 1:
    acc = cmul(M[row][0], x[0])
    for col = 1 .. 2^k - 1:
        acc = cadd(acc, cmul(M[row][col], x[col]))
    amplitude(b | spread(row)) = acc
```

All `x[col]` of a group are read before any amplitude of that group is
written. Different values of `b` address disjoint groups.

### 5.3 No skipped terms

Every term is evaluated, including terms whose matrix entry is `(+0, +0)`.
Skipping a zero term is not an optimization a backend may choose: it can
change the sign of a zero result (`add(-0, +0) = +0`), and therefore the
bytes. A backend MAY use a specialised kernel for a particular matrix only if
it produces the same bytes as section 5.2 for every input; the conformance
suite (section 11) contains cases built to catch the common shortcuts.

Operations are applied in circuit order. A backend MUST NOT fuse, reorder,
cancel or decompose operations: each of these changes the rounded result.

### 5.4 Limits

The reference implementation accepts up to 8 targets and any number of
controls. Suite v1 exercises up to 6 targets. A backend that cannot apply an
operation exactly as section 5.2 defines it MUST reject the operation before
any arithmetic; it MUST NOT approximate or decompose it.

### 5.5 Order of groups and parallelism

Because each group of section 5.2 reads and writes only its own amplitudes,
a backend MAY process groups in any order and in parallel. The reference CPU
backend splits large states into contiguous pieces, each closed under the
operation, and processes the pieces on separate threads; its results do not
depend on the number of threads, which the test suite checks.

---

## 6. Gate matrices

### 6.1 Construction (Tier 2)

Matrices are built on the host. Every component that is not an exact
constant is computed in binary64 by the expression written below, with sine
and cosine from `vikshep-detmath`, then rounded once to binary32 and flushed:

```
r32(x) = ftz(fl32(x))
```

Zero entries are `(+0, +0)` unless written otherwise. In the formulas,
`h = theta * 0.5` in binary64, `C = r32(cos(h))` and `S = r32(sin(h))`;
`-S` is the exact negation of the binary32 value, so for `S = +0` it is `-0`.

### 6.2 Fixed gates

`R = fl32(1 / sqrt(2)) = 0x3F3504F3`. Matrices are written by rows.

| Gate | Matrix |
|---|---|
| `id` | `[(1,+0), (+0,+0)], [(+0,+0), (1,+0)]` |
| `x` | `[(+0,+0), (1,+0)], [(1,+0), (+0,+0)]` |
| `y` | `[(+0,+0), (+0,-1)], [(+0,1), (+0,+0)]` |
| `z` | `[(1,+0), (+0,+0)], [(+0,+0), (-1,+0)]` |
| `h` | `[(R,+0), (R,+0)], [(R,+0), (-R,+0)]` |
| `s` | `diag((1,+0), (+0,1))` |
| `sdg` | `diag((1,+0), (+0,-1))` |
| `t` | `diag((1,+0), (R,R))` |
| `tdg` | `diag((1,+0), (R,-R))` |
| `sx` | `[(0.5,0.5), (0.5,-0.5)], [(0.5,-0.5), (0.5,0.5)]` |
| `swap` | the 4 x 4 permutation exchanging indices 1 and 2: entries `(1,+0)` at (0,0), (1,2), (2,1), (3,3) |

### 6.3 Parametrized gates

Parameters are binary64 angles in radians and MUST be finite.

| Gate | Matrix |
|---|---|
| `rx(theta)` | `[(C,+0), (+0,-S)], [(+0,-S), (C,+0)]` |
| `ry(theta)` | `[(C,+0), (-S,+0)], [(S,+0), (C,+0)]` |
| `rz(theta)` | `diag((C,-S), (C,S))` |
| `p(lambda)` | `diag((1,+0), (r32(cos(lambda)), r32(sin(lambda))))` |

`u(theta, phi, lambda)`, the general single-qubit gate of OpenQASM 3, with
`ch = cos(h)`, `sh = sin(h)`, `cl = cos(lambda)`, `sl = sin(lambda)`,
`cp = cos(phi)`, `sp = sin(phi)`, `cs = cos(fl64(phi + lambda))`,
`ss = sin(fl64(phi + lambda))`, all in binary64:

```
[(r32(ch), +0),          (r32(-(cl * sh)), r32(-(sl * sh)))]
[(r32(cp * sh), r32(sp * sh)),  (r32(cs * ch), r32(ss * ch))]
```

Each product is rounded once in binary64 before `r32`.

### 6.4 Explicit matrices

An explicit matrix on `k` targets is `4^k` complex entries, row-major, each
component finite. Its components are flushed (`ftz`); it is not checked for
unitarity. A matrix supplied in binary64 (for example a numpy `complex128`
array) is rounded once per component to binary32 by the binding before it
reaches the host code of section 6.1.

### 6.5 Controlled gates

A controlled gate is a gate of sections 6.2 to 6.4 with control qubits
(section 5.1). Common names map as follows: `cx`, `cy`, `cz`, `ch`, `cp`,
`crx`, `cry`, `crz` are the named gate with one control; `ccx` is `x` with
two controls; `cswap` is `swap` with one control; `mcx` is `x` with any
number of controls. The control does not change the matrix.

---

## 7. Probabilities

### 7.1 Leaves

The probability of basis state `i` is

```
p[i] = add(mul(re_i, re_i), mul(im_i, im_i))
```

### 7.2 The probability tree

Level 0 of the tree is the `2^n` leaves. Node `j` of level `L`, for
`L = 1 .. n`, is

```
node(L, j) = add(node(L-1, 2j), node(L-1, 2j + 1))
```

The root, `node(n, 0)`, is the squared norm of the state. Expectation values
(section 9) use the same tree shape over their own terms. Any evaluation
order that computes exactly these nodes is conforming; the reference
evaluates it with a stack of `n + 1` partial sums.

---

## 8. Sampling

A sample is drawn by descending the probability tree from the root to a
leaf, one level per qubit, from qubit `n - 1` down to qubit 0. Shots use the
Philox stream `(master_seed, stream_id)` of VDS-1 section 5.3; shot `s`
(counting from 0) consumes words `s * n` to `s * n + n - 1`, one per level,
in descending order of level. At a node of mass `m` whose children have
masses `l` (bit 0) and `r` (bit 1):

```
u   = next_f32_unit()            # (word >> 8) * 2^-24, exact, in [0, 1)
t   = mul(u, m)
bit = 0 if l > 0 and (t < l or r == 0), else 1
```

At level 1 the children are the leaves of section 7.1. The outcome is the
leaf index; bit `q` is the value of qubit `q`.

The squared norm MUST be positive and finite; otherwise sampling is an
error. Masses are non-negative and `m = add(l, r)`, so the rule never enters
a child of zero mass: no outcome of probability zero is ever returned.

The rule is exact as a function of its inputs, not as a probability law:
each decision resolves the split of its node to `2^-24` of the node's mass,
and the tree itself carries binary32 rounding. Outcomes whose probability is
far below `2^-24` relative to their sibling subtrees are not sampled at
their exact rate.

A sample sequence is a function of `(state, master_seed, stream_id)` and the
shot index only: the first `k` shots of a request for `K >= k` shots are the
`k` shots of a request for `k`.

---

## 9. Expectation values of Pauli strings

A Pauli string assigns `I`, `X`, `Y` or `Z` to every qubit. Its text form
lists the non-identity factors with their qubit index, separated by
whitespace (`X0 Z2 Y5`); `I` alone, or the empty string, is the identity.
Let `x_mask` have bit `q` set where qubit `q` carries `X` or `Y`, `z_mask`
where it carries `Z` or `Y`, and `y` be the number of `Y` factors.

For every index `i`, with `j = i XOR x_mask`, `a` = amplitude `i` and `b` =
amplitude `j`:

```
re = add(mul(a.re, b.re), mul(a.im, b.im))        # Re(conj(a) b)
im = sub(mul(a.re, b.im), mul(a.im, b.re))        # Im(conj(a) b)
v  = re, -im, -re, im       for y mod 4 = 0, 1, 2, 3
term[i] = -v if popcount(j AND z_mask) is odd, else v
```

The expectation value is the root of the tree of section 7.2 over
`term[0 .. 2^n]`. It is `<psi|P|psi>` evaluated in binary32; it is not
divided by the squared norm, and its imaginary part, which is zero for a
Hermitian `P`, is not computed.

---

## 10. Records

### 10.1 Circuits and their canonical form (Tier 2)

A circuit is a number of qubits and a list of operations. Operations are
checked when they are added (qubit ranges, distinct qubits, target count,
finite parameters, matrix size and finite entries); a rejected operation
leaves the circuit unchanged.

The canonical form is the RFC 8785 JSON object

```
{"n_qubits": n, "ops": [op, ...], "vqs_version": 1}
```

where each `op` is `{"controls": [...], "gate": name, "params": [...],
"targets": [...]}` and, for an explicit matrix only, a member `"matrix"`
whose value is the lowercase hex of the flushed matrix in the layout of
section 3.2. Gate names are those of section 6 (`unitary` for an explicit
matrix). Each parameter is written as the 16-digit lowercase hex of its
binary64 bit pattern, so `0.0` and `-0.0` are different circuits, as they
are different arithmetic. The circuit hash is the lowercase hex SHA3-256 of
the canonical form.

### 10.2 State identifiers

The OID of a state is the OID of VDS-1 section 9.2 (the first 14 bytes of
the SHA3-256 of the bytes, 28 hex characters) over its canonical layout
(section 3.2).

### 10.3 Run manifest

A run manifest records what was computed. It is the RFC 8785 JSON object
with these members:

| Member | Value |
|---|---|
| `schema` | `"vikshep.quantum.run/1"` |
| `vqs_version`, `numerics_version` | integers |
| `n_qubits` | integer |
| `initial_state` | `{"kind": "zero"}`, or `{"kind": "loaded", "oid": OID}` for a loaded state (OID of its flushed bytes) |
| `circuit_sha3_256` | hash (section 10.1) of the operations applied since the initial state |
| `n_ops` | number of those operations |
| `state` | `{"dtype": "float32", "oid": OID, "shape": [2^n, 2]}` |
| `execution` | `{"backend", "platform", "package_version"}` |
| `manifest_hash` | see below |

`manifest_hash` is the lowercase hex SHA3-256 of the canonical form of the
manifest without its `execution` and `manifest_hash` members. Two conforming
executions of the same run on any backend, platform or thread count
therefore have the same `manifest_hash`. Manifests carry no floating-point
results; the state is referenced by its OID. Version 1 does not record
samples or expectation values; their inputs (seed, stream, shots, Pauli
string) fully determine them.

---

## 11. Conformance

### 11.1 Definition

A backend conforms to VQS-1 at a given `vqs_version` if and only if it
reproduces 100% of that version's conformance cases bit-exactly. Partial
conformance is non-conformance.

### 11.2 Suite v1

Suite `vqs1-v1` has 257 cases. Every input is generated by code from the
Philox stream `(0x56515331, stream id)` with exact operations (random
components are `(2u - 1) * 2^-s` with `u = next_f32_unit()`), so the inputs
are part of the specification and need no data files.

| Family | Cases | What it checks |
|---|---:|---|
| `matrix/` | 6 | host matrices of every named gate, at special angles (signed zeros, multiples of pi, tiny and huge angles) and 256 random angles |
| `gate/` | 90 | each of 15 named single-qubit gates on every target of a 4-qubit state and on qubits 0 and 6 of a 7-qubit state |
| `ctrl/` | 35 | controlled, doubly and multiply controlled gates, swaps in both target orders, controlled swaps |
| `unitary/` | 27 | dense matrices on 1 to 6 targets, random target order, 0 to 2 controls |
| `edge/` | 60 | states and matrices drawn from a pool of boundary values: signed zeros, `FLT_MIN`, subnormals (flushed on load), values whose products round to or just miss `FLT_MIN`, and ordinary values |
| `circuit/` | 15 | Bell, GHZ, quantum Fourier transforms and random circuits of up to 16 qubits and 240 operations; outputs the circuit's canonical form, the final state and the manifest hash |
| `measure/` | 12 | probabilities, squared norm and 512 samples of random, sparse, half-empty, near-underflow, GHZ and circuit-prepared states |
| `expect/` | 12 | 31 Pauli strings on each of the same states |

Each case produces named outputs: `float32` and `uint64` tensors as
little-endian bytes, and `utf8` text.

### 11.3 Expected vectors and runner

`conformance/vectors/v1/expected.json` lists, per case and output, the
dtype, byte length and SHA3-256. It is produced by the CPU reference with

```
cargo run --release -p vikshep-qconformance -- generate
```

and checked with

```
cargo run --release -p vikshep-qconformance -- run [--threads N] [--prefix P] [--report FILE]
```

The report is canonical JSON with, per case, `PASS` or `FAIL`, the output
digests and, for a failure, the first mismatch. It contains no timing or
platform data, so reports from different machines compare byte for byte.
The process exits with status 1 if any case fails. The vectors are compiled
into the library, so the Python module runs the same suite without files
(`vikshep_quantum.conformance_selftest()`).

### 11.4 Cross-platform verification

The CI workflow runs the suite on Linux x86_64, Linux AArch64, macOS arm64
and Windows x86_64 and fails unless every report is all-pass and
byte-identical. The vectors were generated on macOS x86_64. The first CI
run (`vikshep-quantum` Actions run 44176088909, commit 3d053cb) produced
four identical all-pass reports, with SHA-256
`53f352339ee1d98180dd0045ccff5c1470e7efe97f2eefbe0ae2d8ae8969fd3d`, equal to
the macOS x86_64 report.

### 11.5 Correctness

Bit identity shows reproducibility, not correctness. Correctness is checked
separately against independent references, none of which shares code with
the kernels: a binary64 simulator written from the textbook definitions
(Rust and numpy, with their own gate matrices and an out-of-place
definition of controlled operators), the closed form of the quantum Fourier
transform, Pauli expectation values computed from dense operators, sample
frequencies against probabilities, and, when installed, Qiskit's
`Statevector`. A second, independently written implementation of section 5
with the specified arithmetic passes the whole suite, and the same
implementation fails it when its arithmetic omits the flush, judges
multiplication tininess after rounding, or skips zero matrix entries.

---

## 12. Versioning

`vqs_version` identifies the algorithms of sections 3 to 10. Any change
that alters an output byte for a valid input requires a new `vqs_version`
and a new conformance suite, even when the change improves accuracy.
`numerics_version` is VDS-1's; a change there also changes VQS-1 outputs
and requires a new suite. Both versions are recorded in every manifest and
report.

---

## 13. Open decisions register

| Id | Question | Status |
|---|---|---|
| D-Q-PREC | A binary64 Tier-1 mode for deep circuits. Metal has no binary64, so it would be CPU and CUDA only. | Open |
| D-Q-FUSE | A normative fused form of adjacent gates (faster, different rounding). Fusion is forbidden in v1. | Open |
| D-Q-MEAS | Mid-circuit measurement and reset: collapse needs renormalization, which needs division, which VDS-1 forbids in Tier 1 (D-DIV). | Open |
| D-Q-NOISE | Noise channels and quantum trajectories (random choices from Philox streams). | Open |
| D-Q-GRAD | Gradients of expectation values (adjoint method): the order of the reverse pass. | Open |
| D-Q-TN | Tensor-network simulation of circuits too large for a state vector: a normative contraction order. | Open |
| D-Q-REC | Recording samples and expectation values (inputs and result OIDs) in the run manifest. | Open |

---

## 14. Measured accuracy (informative)

Recorded on macOS x86_64 (Intel Core i7-7700HQ); not normative.

- Random circuits over the whole gate set, 1 to 7 qubits, 80 operations
  each, 42 circuits: the largest difference of any amplitude component from
  the binary64 reference was `3.92e-7`.
- Quantum Fourier transform of a 5-qubit basis state: within `5e-6` of the
  closed form (test bound).
- Qiskit 2.6.2 `Statevector`: 15 random circuits of 1 to 8 qubits with 60
  operations each (named gates with up to two controls, and explicit
  matrices on up to three targets) agree within `2e-5` per amplitude, and a
  6-qubit quantum Fourier transform within `1e-6` (test bounds).

binary32 rounding accumulates with circuit depth; these figures describe
these circuits only.
