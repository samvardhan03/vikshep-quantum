"""Behaviour of the Python API: results against an independent numpy
simulator, input-layout independence, reproducibility, errors."""

import json
import math

import numpy as np
import pytest

import vikshep_quantum as vq

# ---------------------------------------------------------------------------
# Independent numpy oracle (binary64, textbook matrices, dense operators).
# ---------------------------------------------------------------------------

R = 1 / math.sqrt(2)
MATRICES = {
    "id": np.eye(2),
    "x": np.array([[0, 1], [1, 0]]),
    "y": np.array([[0, -1j], [1j, 0]]),
    "z": np.diag([1, -1]),
    "h": np.array([[R, R], [R, -R]]),
    "s": np.diag([1, 1j]),
    "sdg": np.diag([1, -1j]),
    "t": np.diag([1, np.exp(1j * math.pi / 4)]),
    "tdg": np.diag([1, np.exp(-1j * math.pi / 4)]),
    "sx": 0.5 * np.array([[1 + 1j, 1 - 1j], [1 - 1j, 1 + 1j]]),
    "swap": np.array([[1, 0, 0, 0], [0, 0, 1, 0], [0, 1, 0, 0], [0, 0, 0, 1]]),
}


def matrix(name, params):
    if name in MATRICES:
        return np.asarray(MATRICES[name], dtype=np.complex128)
    if name == "rx":
        (t,) = params
        return np.array([[math.cos(t / 2), -1j * math.sin(t / 2)], [-1j * math.sin(t / 2), math.cos(t / 2)]])
    if name == "ry":
        (t,) = params
        return np.array([[math.cos(t / 2), -math.sin(t / 2)], [math.sin(t / 2), math.cos(t / 2)]])
    if name == "rz":
        (t,) = params
        return np.diag([np.exp(-0.5j * t), np.exp(0.5j * t)])
    if name == "p":
        (lam,) = params
        return np.diag([1, np.exp(1j * lam)])
    if name == "u":
        t, p, lam = params
        c, s = math.cos(t / 2), math.sin(t / 2)
        return np.array([[c, -np.exp(1j * lam) * s], [np.exp(1j * p) * s, np.exp(1j * (p + lam)) * c]])
    raise KeyError(name)


def operator(n, m, targets, controls):
    """Dense 2^n x 2^n operator of a controlled gate, by its definition."""
    dim = 1 << n
    op = np.zeros((dim, dim), dtype=np.complex128)
    for i in range(dim):
        if any(((i >> c) & 1) == 0 for c in controls):
            op[i, i] = 1
            continue
        row = sum(((i >> t) & 1) << b for b, t in enumerate(targets))
        base = i
        for t in targets:
            base &= ~(1 << t)
        for col in range(1 << len(targets)):
            j = base
            for b, t in enumerate(targets):
                j |= ((col >> b) & 1) << t
            op[i, j] = m[row, col]
    return op


NAMED = ["id", "x", "y", "z", "h", "s", "sdg", "t", "tdg", "sx", "rx", "ry", "rz", "p", "u", "swap"]
ARITY = {"rx": 1, "ry": 1, "rz": 1, "p": 1, "u": 3}


def random_circuit(n, depth, rng):
    """A vikshep circuit and the oracle's final state for it."""
    c = vq.Circuit(n)
    psi = np.zeros(1 << n, dtype=np.complex128)
    psi[0] = 1
    while len(c) < depth:
        if rng.random() < 0.15:
            k = int(rng.integers(1, min(3, n) + 1))
            # A random unitary from the QR decomposition of a complex Gaussian.
            q, _ = np.linalg.qr(rng.normal(size=(1 << k, 1 << k)) + 1j * rng.normal(size=(1 << k, 1 << k)))
            m = q.astype(np.complex64)
            n_ctrl = int(rng.integers(0, min(2, n - k) + 1))
            qubits = [int(v) for v in rng.permutation(n)[: k + n_ctrl]]
            c.unitary(m, qubits[:k], qubits[k:])
            psi = operator(n, m.astype(np.complex128), qubits[:k], qubits[k:]) @ psi
            continue
        name = NAMED[int(rng.integers(len(NAMED)))]
        k = 2 if name == "swap" else 1
        if k > n:
            continue
        params = [float(v) for v in rng.uniform(-2 * math.pi, 2 * math.pi, ARITY.get(name, 0))]
        n_ctrl = int(rng.integers(0, min(2, n - k) + 1))
        qubits = [int(v) for v in rng.permutation(n)[: k + n_ctrl]]
        c.gate(name, qubits[:k], params, qubits[k:])
        psi = operator(n, matrix(name, params), qubits[:k], qubits[k:]) @ psi
    return c, psi


PAULI = {"I": np.eye(2), "X": MATRICES["x"], "Y": MATRICES["y"], "Z": MATRICES["z"]}


def pauli_matrix(n, letters):
    """letters[q] is the factor on qubit q; qubit 0 is the rightmost kron factor."""
    out = np.array([[1.0 + 0j]])
    for q in reversed(range(n)):
        out = np.kron(out, PAULI[letters[q]])
    return out


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("n", [1, 2, 3, 5, 6])
def test_random_circuits_match_the_numpy_oracle(n):
    rng = np.random.default_rng(100 + n)
    for _ in range(4):
        c, psi = random_circuit(n, 40, rng)
        sim = vq.Simulator(n)
        sim.run(c)
        state = sim.state()
        assert state.dtype == np.complex64 and state.shape == (1 << n,)
        assert np.max(np.abs(state.astype(np.complex128) - psi)) < 2e-5
        assert abs(sim.norm_sq() - 1) < 1e-4
        assert np.allclose(sim.probabilities(), np.abs(psi) ** 2, atol=2e-5)


def test_expectations_match_the_numpy_oracle():
    rng = np.random.default_rng(7)
    n = 4
    c, psi = random_circuit(n, 30, rng)
    sim = vq.Simulator(n)
    sim.run(c)
    for _ in range(30):
        letters = [str(v) for v in rng.choice(list("IXYZ"), n)]
        text = " ".join(f"{p}{q}" for q, p in enumerate(letters) if p != "I") or "I"
        want = float(np.real(np.vdot(psi, pauli_matrix(n, letters) @ psi)))
        assert abs(sim.expectation(text) - want) < 5e-6, text


def test_bell_counts_and_reproducible_samples():
    sim = vq.Simulator(2)
    sim.run(vq.Circuit(2).h(0).cx(0, 1))
    samples = sim.sample(2000, seed=7)
    assert samples.dtype == np.uint64
    counts = vq.counts(samples, 2)
    assert set(counts) == {"00", "11"}
    assert abs(counts["00"] - 1000) < 160
    assert np.array_equal(samples, sim.sample(2000, seed=7))
    assert np.array_equal(samples[:100], sim.sample(100, seed=7))
    assert not np.array_equal(samples, sim.sample(2000, seed=7, stream=1))


def test_methods_chain_and_record_operations():
    c = vq.Circuit(3)
    assert c.h(0).cx(0, 1).rz(0.5, 2).ccx(0, 1, 2) is c
    assert len(c) == 4
    assert c.ops()[1] == ("x", [], [1], [0])
    assert c.ops()[2] == ("rz", [0.5], [2], [])
    copy = c.copy().x(0)
    assert len(c) == 4 and len(copy) == 5
    assert c.hash() != copy.hash()
    sim = vq.Simulator(3)
    sim.run(c)
    assert sim.applied.hash() == c.hash()


def test_input_layout_and_precision_do_not_change_results():
    rng = np.random.default_rng(3)
    q, _ = np.linalg.qr(rng.normal(size=(4, 4)) + 1j * rng.normal(size=(4, 4)))
    variants = [
        q.astype(np.complex64),
        np.asfortranarray(q.astype(np.complex64)),
        q,  # complex128: each component rounded once to binary32
        q.astype(np.complex64).astype(">c8"),  # big-endian
        q.astype(np.complex64).tolist(),
    ]
    hashes = {vq.Circuit(2).unitary(m, [0, 1]).hash() for m in variants}
    assert len(hashes) == 1

    amps = (rng.normal(size=16) + 1j * rng.normal(size=16)).astype(np.complex64)
    oids = set()
    for a in [amps, np.repeat(amps, 2)[::2], amps.astype(">c8"), amps.astype(np.complex128)]:
        sim = vq.Simulator(4)
        sim.load_state(a)
        oids.add(sim.state_oid())
    assert len(oids) == 1


def test_returned_arrays_are_copies():
    sim = vq.Simulator(1)
    s = sim.state()
    s[0] = 0
    assert sim.state()[0] == 1


def test_thread_count_and_manifest_hash():
    rng = np.random.default_rng(11)
    c, _ = random_circuit(4, 20, rng)
    big = vq.Circuit(18)
    for name, params, targets, controls in c.ops():
        if name != "unitary":
            big.gate(name, [t + 14 for t in targets], params, [q + 14 for q in controls])
    for q in range(18):
        big.h(q)
    results = []
    for threads in (1, 3, 8):
        sim = vq.Simulator(18, threads=threads)
        assert sim.threads == threads
        sim.run(big)
        m = json.loads(sim.manifest())
        results.append((sim.state_oid(), sim.sample(64, seed=1).tobytes(), m["manifest_hash"]))
        assert m["execution"]["backend"] == "cpu"
        assert m["n_qubits"] == 18 and m["n_ops"] == len(big)
    assert results[0] == results[1] == results[2]


def test_load_state_changes_the_manifest_but_not_the_state_bytes():
    a = vq.Simulator(2)
    b = vq.Simulator(2)
    b.load_state(np.array([1, 0, 0, 0], dtype=np.complex64))
    assert a.state_oid() == b.state_oid()
    ma, mb = json.loads(a.manifest()), json.loads(b.manifest())
    assert ma["initial_state"] == {"kind": "zero"}
    assert mb["initial_state"]["kind"] == "loaded"
    assert ma["manifest_hash"] != mb["manifest_hash"]


def test_invalid_input_is_rejected():
    c = vq.Circuit(2)
    with pytest.raises(ValueError, match="out of range"):
        c.h(2)
    with pytest.raises(ValueError, match="more than once"):
        c.cx(1, 1)
    with pytest.raises(ValueError, match="unknown gate"):
        c.gate("frobnicate", [0])
    with pytest.raises(ValueError, match="parameter"):
        c.gate("rx", [0])
    with pytest.raises(ValueError, match="finite"):
        c.rx(float("nan"), 0)
    with pytest.raises(ValueError, match="shape"):
        c.unitary(np.eye(2), [0, 1])
    assert len(c) == 0
    with pytest.raises(ValueError):
        vq.Circuit(0)
    with pytest.raises(ValueError):
        vq.Circuit(vq.MAX_QUBITS + 1)
    sim = vq.Simulator(2)
    with pytest.raises(ValueError, match="circuit has 3 qubits"):
        sim.run(vq.Circuit(3))
    with pytest.raises(ValueError, match="shape"):
        sim.load_state(np.zeros(3, dtype=np.complex64))
    with pytest.raises(ValueError, match="Pauli"):
        sim.expectation("Q0")
    sim.load_state(np.zeros(4, dtype=np.complex64))
    with pytest.raises(ValueError, match="norm"):
        sim.sample(1, seed=0)


def test_oid_matches_the_state_bytes():
    sim = vq.Simulator(3)
    sim.run(vq.Circuit(3).h(0).cx(0, 2))
    assert vq.oid(sim.state().astype("<c8").tobytes()) == sim.state_oid()
    assert len(sim.state_oid()) == 28
