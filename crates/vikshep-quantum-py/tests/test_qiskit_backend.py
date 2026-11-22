"""The Qiskit integration (``vikshep_quantum.qiskit``), checked against
Qiskit's own reference implementations: ``Statevector``, ``BasicSimulator``,
``StatevectorSampler`` and ``StatevectorEstimator``. Skipped when qiskit is
not installed."""

import json
import math
import warnings
from collections import Counter

import numpy as np
import pytest

import vikshep_quantum as vq

pytest.importorskip("qiskit")
from qiskit import ClassicalRegister, QuantumCircuit, QuantumRegister, transpile  # noqa: E402
from qiskit.circuit import Gate, Parameter  # noqa: E402
from qiskit.circuit.library import PauliEvolutionGate, QFTGate, UnitaryGate, efficient_su2  # noqa: E402
from qiskit.circuit.library.standard_gates import get_standard_gate_name_mapping  # noqa: E402
from qiskit.circuit.random import random_circuit  # noqa: E402
from qiskit.exceptions import QiskitError  # noqa: E402
from qiskit.primitives import StatevectorEstimator, StatevectorSampler  # noqa: E402
from qiskit.providers.basic_provider import BasicSimulator  # noqa: E402
from qiskit.quantum_info import SparseObservable, SparsePauliOp, Statevector, random_unitary  # noqa: E402

from vikshep_quantum.qiskit import Estimator, Sampler, VikshepSimulator, from_qiskit  # noqa: E402

GATES = get_standard_gate_name_mapping()
STANDARD = sorted(set(GATES) - {"measure", "reset", "delay", "global_phase"})


def our_state(circuit):
    sim = vq.Simulator(circuit.num_qubits)
    sim.run(from_qiskit(circuit))
    return sim.state().astype(np.complex128)


def assert_same_state(circuit):
    """Same state as Qiskit's Statevector, up to the dropped global phase."""
    want = Statevector(circuit.remove_final_measurements(inplace=False)).data
    got = our_state(circuit)
    overlap = np.vdot(want, got)
    assert abs(abs(overlap) - 1) < 2e-6, abs(overlap)
    assert np.max(np.abs(got - overlap / abs(overlap) * want)) < 1e-5


def scramble(qc, rng):
    """Random single-qubit rotations and a CX chain, for a generic state."""
    for q in range(qc.num_qubits):
        qc.u(*rng.uniform(-math.pi, math.pi, 3), q)
    for q in range(qc.num_qubits - 1):
        qc.cx(q, q + 1)


def instance(name, rng):
    gate = GATES[name]
    return gate.base_class(*rng.uniform(-2 * math.pi, 2 * math.pi, len(gate.params)))


# --- conversion -----------------------------------------------------------


@pytest.mark.parametrize("name", STANDARD)
def test_every_standard_gate(name):
    rng = np.random.default_rng(sum(map(ord, name)))
    gate = instance(name, rng)
    n = gate.num_qubits + 2
    qc = QuantumCircuit(n)
    scramble(qc, rng)
    qc.append(gate, [int(q) for q in rng.permutation(n)[: gate.num_qubits]])
    assert_same_state(qc)


@pytest.mark.parametrize("name", STANDARD)
def test_every_standard_gate_with_open_and_closed_controls(name):
    rng = np.random.default_rng(1000 + sum(map(ord, name)))
    gate = instance(name, rng).control(2, ctrl_state=int(rng.integers(4)), annotated=False)
    n = gate.num_qubits + 1
    qc = QuantumCircuit(n)
    scramble(qc, rng)
    qc.append(gate, [int(q) for q in rng.permutation(n)[: gate.num_qubits]])
    assert_same_state(qc)


def test_vqs1_gates_become_one_operation_with_the_same_parameters():
    qc = QuantumCircuit(4)
    qc.rz(0.1, 0)
    qc.crx(0.2, 1, 2)
    qc.mcx([0, 1, 2], 3)
    qc.cp(0.3, 3, 0)
    qc.u(0.4, 0.5, 0.6, 1)
    qc.append(GATES["u3"].base_class(0.7, 0.8, 0.9), [2])
    qc.append(UnitaryGate(random_unitary(4, seed=1)), [2, 0])
    ops = from_qiskit(qc).ops()
    assert ops[:6] == [
        ("rz", [0.1], [0], []),
        ("rx", [0.2], [2], [1]),
        ("x", [], [3], [0, 1, 2]),
        ("p", [0.3], [0], [3]),
        ("u", [0.4, 0.5, 0.6], [1], []),
        ("u", [0.7, 0.8, 0.9], [2], []),
    ]
    assert ops[6][0] == "unitary" and ops[6][2] == [2, 0]
    assert len(ops) == 7


def test_definitions_pass_angles_through_unchanged():
    theta = 0.123456789
    qc = QuantumCircuit(2)
    qc.rzz(theta, 0, 1)
    ops = from_qiskit(qc).ops()
    assert [op[0] for op in ops] == ["x", "rz", "x"]
    assert ops[1][1] == [theta]


def test_library_circuits():
    rng = np.random.default_rng(5)
    qft = QuantumCircuit(5)
    scramble(qft, rng)
    qft.append(QFTGate(5), range(5))
    assert_same_state(qft)

    # Commuting terms: the gate's definition (one Trotter step) is then the
    # exact exponential that Statevector uses.
    evo = QuantumCircuit(3)
    scramble(evo, rng)
    evo.append(PauliEvolutionGate(SparsePauliOp(["XXI", "IXX", "XIX"], [0.3, -0.7, 0.2]), time=0.9), range(3))
    assert_same_state(evo)

    ansatz = efficient_su2(5, reps=3)
    assert_same_state(ansatz.assign_parameters(rng.uniform(-3, 3, ansatz.num_parameters)))

    for seed in range(5):
        assert_same_state(random_circuit(6, 12, max_operands=3, seed=seed))


def test_initialize_at_the_start():
    amps = np.array([0.5, 0.5j, -0.5, 0.5])
    qc = QuantumCircuit(2)
    qc.initialize(amps, [0, 1])
    assert_same_state(qc)


def test_unsupported_circuits_are_rejected():
    mid = QuantumCircuit(2, 2)
    mid.h(0)
    mid.measure(0, 0)
    mid.x(0)
    reset = QuantumCircuit(1)
    reset.h(0)
    reset.reset(0)
    unbound = QuantumCircuit(1)
    unbound.rx(Parameter("a"), 0)
    flow = QuantumCircuit(1, 1)
    flow.measure(0, 0)
    with flow.if_test((flow.clbits[0], 1)):
        flow.x(0)
    opaque = QuantumCircuit(1)
    opaque.append(Gate("mystery", 1, []), [0])
    for qc, message in [
        (mid, "after it was measured"),
        (reset, "reset of qubit 0"),
        (unbound, "unbound parameter"),
        (flow, "control flow"),
        (opaque, "no definition"),
    ]:
        with pytest.raises(QiskitError, match=message):
            from_qiskit(qc)


def test_gates_on_other_qubits_may_follow_a_measurement():
    qc = QuantumCircuit(2, 2)
    qc.x(0)
    qc.measure(0, 0)
    qc.x(1)
    qc.barrier()
    qc.measure(1, 1)
    counts = VikshepSimulator().run(qc, shots=10).result().get_counts()
    assert counts == {"11": 10}


# --- backend --------------------------------------------------------------


def test_counts_match_basic_simulator_on_deterministic_circuits():
    qr = QuantumRegister(4)
    a = ClassicalRegister(3, "a")
    b = ClassicalRegister(2, "b")
    qc = QuantumCircuit(qr, a, b)
    qc.x([0, 2, 3])
    qc.measure(0, a[2])
    qc.measure(1, a[0])
    qc.measure(2, b[0])
    qc.measure(3, a[0])  # the last measurement into a bit wins
    # a[1] and b[1] are never measured.
    ours = VikshepSimulator().run(qc, shots=50).result().get_counts()
    assert ours == BasicSimulator().run(qc, shots=50).result().get_counts()
    assert ours == {"01 101": 50}

    plain = QuantumCircuit(2, 1)
    plain.h(0)
    assert VikshepSimulator().run(plain, shots=7).result().get_counts() == {"0": 7}
    assert VikshepSimulator().run(QuantumCircuit(1), shots=7).result().get_counts() == {}


def test_memory_is_the_native_sampler_with_one_stream_per_circuit():
    qc = random_circuit(5, 10, max_operands=2, seed=3)
    qc.measure_all()
    result = VikshepSimulator().run([qc, qc], shots=300, seed_simulator=11, memory=True).result()
    sim = vq.Simulator(5)
    sim.run(from_qiskit(qc))
    for i in range(2):
        data = result.results[i].data
        assert data.memory == [hex(v) for v in sim.sample(300, 11, i).tolist()]
        assert data.counts == dict(Counter(data.memory))
        assert result.results[i].metadata["stream"] == i
        assert result.results[i].seed_simulator == 11
    assert result.results[0].data.memory != result.results[1].data.memory


def test_frequencies_follow_the_probabilities():
    qc = random_circuit(4, 8, max_operands=2, seed=8)
    want = Statevector(qc).probabilities_dict()
    qc.measure_all()
    shots = 40000
    counts = VikshepSimulator().run(qc, shots=shots, seed_simulator=2).result().get_counts()
    tvd = 0.5 * sum(abs(counts.get(k, 0) / shots - want.get(k, 0)) for k in set(want) | set(counts))
    assert tvd < 0.02, tvd


def test_seeds_threads_and_manifest():
    qc = efficient_su2(14, reps=2)
    qc = qc.assign_parameters(np.random.default_rng(4).uniform(-3, 3, qc.num_parameters))
    qc.measure_all()
    one = VikshepSimulator(threads=1).run(qc, shots=500, seed_simulator=9, memory=True).result()
    many = VikshepSimulator().run(qc, shots=500, seed_simulator=9, memory=True, threads=4).result()
    assert one.results[0].data.memory == many.results[0].data.memory
    m1 = json.loads(one.results[0].metadata["manifest"])
    m4 = json.loads(many.results[0].metadata["manifest"])
    assert m1["manifest_hash"] == m4["manifest_hash"]
    sim = vq.Simulator(14)
    sim.run(from_qiskit(qc))
    assert m1["manifest_hash"] == json.loads(sim.manifest())["manifest_hash"]

    unseeded = VikshepSimulator().run(qc, shots=100, memory=True).result()
    seed = unseeded.results[0].seed_simulator
    replay = VikshepSimulator().run(qc, shots=100, memory=True, seed_simulator=seed).result()
    assert replay.results[0].data.memory == unseeded.results[0].data.memory


def test_options_and_target():
    backend = VikshepSimulator(shots=10)
    qc = QuantumCircuit(1, 1)
    qc.x(0)
    qc.measure(0, 0)
    assert backend.run(qc).result().get_counts() == {"1": 10}
    assert backend.run(qc, shots=3).result().get_counts() == {"1": 3}
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        backend.run(qc, nonsense=1)
    assert any("nonsense" in str(w.message) for w in caught)
    with pytest.raises(QiskitError, match="seed"):
        backend.run(qc, seed_simulator=-1)
    assert backend.target.num_qubits is None
    for name in ("cx", "rz", "u", "unitary", "mcx", "measure"):
        assert name in backend.target.operation_names


def test_transpiling_keeps_the_width_and_the_vqs1_gates():
    backend = VikshepSimulator()
    qc = QuantumCircuit(3)
    qc.h(0)
    qc.crz(0.4, 0, 2)
    qc.ccx(0, 1, 2)
    qc.measure_all()
    out = transpile(qc, backend, optimization_level=0)
    assert out.num_qubits == 3
    assert out.count_ops() == qc.count_ops()

    exotic = QuantumCircuit(4)
    exotic.append(QFTGate(4), range(4))
    exotic.ecr(0, 1)
    exotic.iswap(2, 3)
    for level in range(4):
        out = transpile(exotic, backend, optimization_level=level, seed_transpiler=1)
        assert out.num_qubits == 4
        assert set(out.count_ops()) <= set(backend.target.operation_names) | {"barrier"}
        assert_same_state(out)


# --- primitives -----------------------------------------------------------


def bell_with_angle():
    theta = Parameter("theta")
    qc = QuantumCircuit(2)
    qc.ry(theta, 0)
    qc.cx(0, 1)
    qc.measure_all()
    return qc


def test_sampler_shapes_registers_and_metadata():
    qc = bell_with_angle()
    result = Sampler(seed=3).run([(qc, np.linspace(0, math.pi, 6).reshape(2, 3))], shots=400).result()
    meas = result[0].data.meas
    assert meas.shape == (2, 3) and meas.num_shots == 400 and meas.num_bits == 2
    assert meas.get_counts((0, 0)) == {"00": 400}
    assert meas.get_counts((1, 2)) == {"11": 400}
    md = result[0].metadata
    assert md["seed"] == 3 and md["streams"] == [0, 6] and md["shots"] == 400
    assert len(md["manifest_hashes"]) == 6


def test_sampler_agrees_with_the_backend_stream_for_stream():
    qc = random_circuit(5, 10, max_operands=2, seed=12)
    qc.measure_all()
    sampled = Sampler(seed=21).run([qc, qc], shots=500).result()
    run = VikshepSimulator().run([qc, qc], shots=500, seed_simulator=21).result()
    for i in range(2):
        assert sampled[i].data.meas.get_counts() == run.get_counts(i)
    assert sampled[1].metadata["streams"] == [1, 2]


def test_sampler_bit_layout_matches_statevector_sampler():
    qr = QuantumRegister(4)
    a = ClassicalRegister(3, "a")
    b = ClassicalRegister(2, "b")
    qc = QuantumCircuit(qr, a, b)
    qc.x([0, 3])
    qc.measure([0, 1, 3], a)
    qc.measure([3, 0], b)
    ours = Sampler(seed=1).run([qc], shots=20).result()[0].data
    ref = StatevectorSampler(seed=1).run([qc], shots=20).result()[0].data
    for name in ("a", "b"):
        assert getattr(ours, name).get_bitstrings() == getattr(ref, name).get_bitstrings()


def test_sampler_replays_a_random_seed():
    qc = random_circuit(4, 6, max_operands=2, seed=2)
    qc.measure_all()
    first = Sampler().run([qc], shots=200).result()[0]
    again = Sampler(seed=first.metadata["seed"]).run([qc], shots=200).result()[0]
    assert first.data.meas.get_bitstrings() == again.data.meas.get_bitstrings()


HAMILTONIAN = SparsePauliOp(["ZZIII", "IXXII", "YIIYI", "IIIIZ", "XYZXY"], [0.5, -1.2, 0.3, 0.7, 0.05])


def test_estimator_matches_statevector_estimator_with_broadcasting():
    ansatz = efficient_su2(5, reps=2)
    values = np.random.default_rng(6).uniform(-3, 3, (3, 1, ansatz.num_parameters))
    observables = [[HAMILTONIAN, SparsePauliOp("XXXXX"), "IIZIZ"]]
    ours = Estimator().run([(ansatz, observables, values)]).result()[0]
    ref = StatevectorEstimator().run([(ansatz, observables, values)]).result()[0]
    assert ours.data.evs.shape == (3, 3)
    assert np.max(np.abs(ours.data.evs - ref.data.evs)) < 1e-5
    assert np.all(ours.data.stds == 0)
    assert len(ours.metadata["manifest_hashes"]) == 3


def test_estimator_sum_order_is_sorted_labels_in_binary64():
    ansatz = efficient_su2(5, reps=2)
    values = np.random.default_rng(7).uniform(-3, 3, ansatz.num_parameters)
    ev = Estimator().run([(ansatz, HAMILTONIAN, values)]).result()[0].data.evs
    sim = vq.Simulator(5)
    sim.run(from_qiskit(ansatz.assign_parameters(values)))
    terms = sorted(zip(HAMILTONIAN.paulis.to_labels(), HAMILTONIAN.coeffs.real, strict=True))
    want = 0.0
    for label, coeff in terms:
        pauli = " ".join(f"{ch}{q}" for q, ch in enumerate(reversed(label)) if ch != "I") or "I"
        want += float(coeff) * float(sim.expectation(pauli))
    assert float(ev) == want
    one = Estimator(threads=1).run([(ansatz, HAMILTONIAN, values)]).result()[0].data.evs
    assert float(one) == want


def test_estimator_projectors_and_identity():
    ansatz = efficient_su2(4, reps=2)
    values = np.random.default_rng(8).uniform(-3, 3, ansatz.num_parameters)
    ours = Estimator().run([(ansatz, ["0+1r", "IIII"], values)]).result()[0].data.evs
    state = Statevector(ansatz.assign_parameters(values))
    paulis = SparseObservable("0+1r").as_paulis()
    want = state.expectation_value(SparsePauliOp.from_sparse_list(paulis.to_sparse_list(), 4)).real
    assert abs(ours[0] - want) < 1e-6
    assert abs(ours[1] - 1) < 1e-6


def test_estimator_rejects_a_nonzero_precision():
    with pytest.raises(ValueError, match="precision"):
        Estimator().run([(efficient_su2(2, reps=1), "ZZ", np.zeros(8))], precision=0.01)
    with pytest.raises(ValueError, match="precision"):
        Estimator(default_precision=0.1)
