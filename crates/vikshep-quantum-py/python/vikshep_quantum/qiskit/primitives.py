"""Qiskit V2 primitives on the VQS-1 CPU reference: ``Sampler`` and ``Estimator``."""

from __future__ import annotations

import json
import warnings
from collections.abc import Iterable

import numpy as np
from qiskit.primitives import BaseEstimatorV2, BaseSamplerV2
from qiskit.primitives.containers import BitArray, DataBin, PrimitiveResult, PubResult, SamplerPubResult
from qiskit.primitives.containers.estimator_pub import EstimatorPub
from qiskit.primitives.containers.sampler_pub import SamplerPub
from qiskit.primitives.primitive_job import PrimitiveJob

import vikshep_quantum as vq

from ._convert import convert
from .backend import _check_seed, _random_seed

_PAULIS = set("IXYZ")


def _fresh(sim: vq.Simulator | None, n_qubits: int, threads: int | None) -> vq.Simulator:
    """A simulator at |0...0>, reusing ``sim``'s memory when there is one."""
    if sim is None:
        return vq.Simulator(n_qubits, threads=threads)
    sim.reset()
    return sim


def _manifest_hash(sim: vq.Simulator) -> str:
    return json.loads(sim.manifest())["manifest_hash"]


class Sampler(BaseSamplerV2):
    """Sampler V2 with bit-identical seeded shots on every platform.

    The ``k``-th circuit run in one ``run`` call (pubs in order, parameter
    bindings of each pub in C order) draws its shots from the Philox stream
    ``(seed, k)``. With ``seed=None`` each ``run`` call picks a random seed;
    it is reported in each pub result's ``metadata["seed"]``. Circuits need
    not be transpiled. Measurements must come at the end of the circuit.

    Each pub result's metadata also has ``streams`` (the first and one past
    the last stream used) and ``manifest_hashes`` (one per parameter
    binding, in C order).
    """

    def __init__(self, *, default_shots: int = 1024, seed: int | None = None, threads: int | None = None):
        self._default_shots = default_shots
        self._seed = None if seed is None else _check_seed(seed)
        self._threads = threads

    @property
    def default_shots(self) -> int:
        return self._default_shots

    @property
    def seed(self) -> int | None:
        return self._seed

    def run(self, pubs: Iterable, *, shots: int | None = None) -> PrimitiveJob[PrimitiveResult[SamplerPubResult]]:
        if shots is None:
            shots = self._default_shots
        coerced = [SamplerPub.coerce(pub, shots) for pub in pubs]
        if any(len(pub.circuit.cregs) == 0 for pub in coerced):
            warnings.warn(
                "One of your circuits has no output classical registers and so the result "
                "will be empty. Did you mean to add measurement instructions?",
                UserWarning,
                stacklevel=2,
            )
        seed = _random_seed() if self._seed is None else self._seed
        job = PrimitiveJob(self._run, coerced, seed)
        job._submit()
        return job

    def _run(self, pubs: list[SamplerPub], seed: int) -> PrimitiveResult[SamplerPubResult]:
        results = []
        stream = 0
        for pub in pubs:
            result, stream = self._run_pub(pub, seed, stream)
            results.append(result)
        return PrimitiveResult(results, metadata={"version": 2})

    def _run_pub(self, pub: SamplerPub, seed: int, stream: int):
        circuit = pub.circuit
        clbit_index = {bit: i for i, bit in enumerate(circuit.clbits)}
        registers = [(creg.name, [clbit_index[bit] for bit in creg]) for creg in circuit.cregs]
        bound = pub.parameter_values.bind_all(circuit)
        bits = {name: np.zeros(bound.shape + (pub.shots, len(idx)), dtype=bool) for name, idx in registers}
        first = stream
        hashes = []
        sim = None
        for index, bound_circuit in np.ndenumerate(bound):
            converted = convert(bound_circuit)
            sim = _fresh(sim, circuit.num_qubits, self._threads)
            sim.run(converted.circuit)
            hashes.append(_manifest_hash(sim))
            samples = sim.sample(pub.shots, seed, stream) if converted.measured else None
            stream += 1
            for name, idx in registers:
                for j, clbit in enumerate(idx):
                    qubit = converted.measured.get(clbit)
                    if qubit is not None:
                        bits[name][index + (slice(None), j)] = (samples >> np.uint64(qubit)) & np.uint64(1)
        data = {name: BitArray.from_bool_array(bits[name], order="little") for name, _ in registers}
        metadata = {
            "shots": pub.shots,
            "circuit_metadata": circuit.metadata,
            "seed": seed,
            "streams": [first, stream],
            "manifest_hashes": hashes,
        }
        return SamplerPubResult(DataBin(**data, shape=pub.shape), metadata=metadata), stream


def _pauli_terms(observable: dict) -> list[tuple[str, float]]:
    """``(VQS-1 Pauli string, coefficient)`` pairs, sorted by Qiskit label.

    Qiskit labels put qubit 0 rightmost. Projector terms (``0``, ``1``,
    ``+``, ``-``, ``r``, ``l``) are first rewritten as Pauli sums by Qiskit.
    """
    if any(set(label) - _PAULIS for label in observable):
        from qiskit.quantum_info import SparseObservable

        paulis = SparseObservable.from_list(list(observable.items())).as_paulis().simplify()
        n = paulis.num_qubits
        observable = {}
        for chars, indices, coeff in paulis.to_sparse_list():
            label = ["I"] * n
            for ch, q in zip(chars, indices, strict=True):
                label[n - 1 - q] = ch
            observable["".join(label)] = complex(coeff).real
    terms = []
    for label in sorted(observable):
        n = len(label)
        ops = [f"{ch}{n - 1 - pos}" for pos, ch in enumerate(label) if ch != "I"]
        terms.append((" ".join(reversed(ops)) or "I", float(np.real(observable[label]))))
    return terms


def _expectation(sim: vq.Simulator, terms: list[tuple[str, float]]) -> float:
    total = 0.0
    for pauli, coeff in terms:
        total += coeff * float(sim.expectation(pauli))
    return total


class Estimator(BaseEstimatorV2):
    """Estimator V2 with exact, bit-identical expectation values.

    An observable ``sum_k c_k P_k`` is evaluated as ``sum_k c_k <P_k>``:
    each ``<P_k>`` is a VQS-1 binary32 expectation value (section 7.4), and
    the sum is taken in binary64, term by term, in sorted label order. The
    result is the same on every platform and for every thread count.

    The values are exact (no shot noise), so the precision must be 0. For
    shot-based estimates use :class:`Sampler`. Each pub result's metadata
    has ``manifest_hashes``, one per parameter binding, in C order.
    """

    def __init__(self, *, default_precision: float = 0.0, threads: int | None = None):
        if default_precision != 0:
            raise ValueError("Estimator values are exact; default_precision must be 0")
        self._default_precision = 0.0
        self._threads = threads

    @property
    def default_precision(self) -> float:
        return self._default_precision

    def run(self, pubs: Iterable, *, precision: float | None = None) -> PrimitiveJob[PrimitiveResult[PubResult]]:
        if precision is None:
            precision = self._default_precision
        coerced = [EstimatorPub.coerce(pub, precision) for pub in pubs]
        for pub in coerced:
            if pub.precision != 0:
                raise ValueError(
                    f"Estimator values are exact; precision must be 0, got {pub.precision} "
                    "(use Sampler for shot-based estimates)"
                )
        job = PrimitiveJob(self._run, coerced)
        job._submit()
        return job

    def _run(self, pubs: list[EstimatorPub]) -> PrimitiveResult[PubResult]:
        return PrimitiveResult([self._run_pub(pub) for pub in pubs], metadata={"version": 2})

    def _run_pub(self, pub: EstimatorPub) -> PubResult:
        circuit = pub.circuit
        bound = pub.parameter_values.bind_all(circuit)
        observables = pub.observables
        shape = np.broadcast_shapes(bound.shape, observables.shape)
        which = np.broadcast_to(np.arange(bound.size).reshape(bound.shape), shape)
        obs_index = np.broadcast_to(np.arange(observables.size).reshape(observables.shape), shape)
        flat_obs = observables.reshape(-1)
        positions: dict[int, list[tuple]] = {}
        for index in np.ndindex(*shape):
            positions.setdefault(int(which[index]), []).append(index)

        evs = np.zeros(shape, dtype=np.float64)
        hashes = []
        sim = None
        for k, bound_circuit in enumerate(bound.reshape(-1)):
            converted = convert(bound_circuit)
            sim = _fresh(sim, circuit.num_qubits, self._threads)
            sim.run(converted.circuit)
            hashes.append(_manifest_hash(sim))
            for index in positions.get(k, []):
                evs[index] = _expectation(sim, _pauli_terms(flat_obs[int(obs_index[index])]))
        data = DataBin(evs=evs, stds=np.zeros(shape, dtype=np.float64), shape=shape)
        return PubResult(
            data,
            metadata={
                "target_precision": 0.0,
                "circuit_metadata": circuit.metadata,
                "manifest_hashes": hashes,
            },
        )
