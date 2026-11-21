"""Cross-check against Qiskit's Statevector (skipped when qiskit is not
installed). Qiskit is a test-only reference and is not a dependency."""

import math

import numpy as np
import pytest

import vikshep_quantum as vq

qiskit = pytest.importorskip("qiskit")
from qiskit import QuantumCircuit  # noqa: E402
from qiskit.circuit import library as lib  # noqa: E402
from qiskit.quantum_info import Statevector  # noqa: E402

QISKIT_GATES = {
    "id": lambda p: lib.IGate(),
    "x": lambda p: lib.XGate(),
    "y": lambda p: lib.YGate(),
    "z": lambda p: lib.ZGate(),
    "h": lambda p: lib.HGate(),
    "s": lambda p: lib.SGate(),
    "sdg": lambda p: lib.SdgGate(),
    "t": lambda p: lib.TGate(),
    "tdg": lambda p: lib.TdgGate(),
    "sx": lambda p: lib.SXGate(),
    "rx": lambda p: lib.RXGate(p[0]),
    "ry": lambda p: lib.RYGate(p[0]),
    "rz": lambda p: lib.RZGate(p[0]),
    "p": lambda p: lib.PhaseGate(p[0]),
    "u": lambda p: lib.UGate(*p),
    "swap": lambda p: lib.SwapGate(),
}
ARITY = {"rx": 1, "ry": 1, "rz": 1, "p": 1, "u": 3}


def build(n, depth, rng):
    ours = vq.Circuit(n)
    theirs = QuantumCircuit(n)
    names = list(QISKIT_GATES)
    while len(ours) < depth:
        if rng.random() < 0.15:
            k = int(rng.integers(1, min(3, n) + 1))
            q, _ = np.linalg.qr(rng.normal(size=(1 << k, 1 << k)) + 1j * rng.normal(size=(1 << k, 1 << k)))
            m = q.astype(np.complex64)
            qubits = [int(v) for v in rng.permutation(n)[:k]]
            ours.unitary(m, qubits)
            # Qiskit also takes the first listed qubit as the low matrix bit.
            theirs.append(lib.UnitaryGate(m.astype(np.complex128), check_input=False), qubits)
            continue
        name = names[int(rng.integers(len(names)))]
        k = 2 if name == "swap" else 1
        if k > n:
            continue
        params = [float(v) for v in rng.uniform(-2 * math.pi, 2 * math.pi, ARITY.get(name, 0))]
        n_ctrl = int(rng.integers(0, min(2, n - k) + 1))
        qubits = [int(v) for v in rng.permutation(n)[: k + n_ctrl]]
        targets, controls = qubits[:k], qubits[k:]
        ours.gate(name, targets, params, controls)
        gate = QISKIT_GATES[name](params)
        if controls:
            gate = gate.control(len(controls), annotated=False)
        theirs.append(gate, controls + targets)
    return ours, theirs


@pytest.mark.parametrize("n", [1, 2, 4, 6, 8])
def test_states_match_qiskit(n):
    rng = np.random.default_rng(900 + n)
    for _ in range(3):
        ours, theirs = build(n, 60, rng)
        sim = vq.Simulator(n)
        sim.run(ours)
        want = Statevector.from_instruction(theirs).data
        diff = np.max(np.abs(sim.state().astype(np.complex128) - want))
        assert diff < 2e-5, diff


def test_qft_matches_qiskit():
    n = 6
    ours = vq.Circuit(n)
    for j in reversed(range(n)):
        ours.h(j)
        for k in reversed(range(j)):
            ours.cp(math.pi / (1 << (j - k)), k, j)
    for q in range(n // 2):
        ours.swap(q, n - 1 - q)
    sim = vq.Simulator(n)
    sim.run(ours)
    theirs = lib.QFTGate(n).definition
    want = Statevector.from_instruction(theirs).data
    assert np.max(np.abs(sim.state() - want)) < 1e-6
