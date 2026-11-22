"""``VikshepSimulator``: a Qiskit ``BackendV2`` on the VQS-1 CPU reference."""

from __future__ import annotations

import secrets
import time
import uuid
import warnings

import numpy as np
from qiskit.circuit import QuantumCircuit
from qiskit.circuit.library import MCPhaseGate, MCXGate, UnitaryGate
from qiskit.circuit.library.standard_gates import get_standard_gate_name_mapping
from qiskit.exceptions import QiskitError
from qiskit.providers import BackendV2, JobStatus, JobV1, Options
from qiskit.result import Result
from qiskit.transpiler import Target

import vikshep_quantum as vq

from ._convert import convert

# Instructions of the target: the gates that convert to one VQS-1 operation,
# plus measure, reset (accepted before any gate) and delay. Gates outside the
# target still run (``run`` expands them with their definitions); the target
# only tells the transpiler which gates to keep.
_TARGET_GATES = [
    "id", "x", "y", "z", "h", "s", "sdg", "t", "tdg", "sx",
    "rx", "ry", "rz", "p", "u", "swap",
    "cx", "cy", "cz", "ch", "cs", "csdg", "csx", "cp", "crx", "cry", "crz",
    "ccx", "ccz", "cswap", "c3sx",
    "measure", "reset", "delay", "global_phase",
]  # fmt: skip

_SEED_LIMIT = 1 << 64


def _build_target() -> Target:
    target = Target(description="Vikshep Quantum CPU reference (VQS-1)", num_qubits=None)
    gates = get_standard_gate_name_mapping()
    for name in _TARGET_GATES:
        target.add_instruction(gates[name], properties=None, name=name)
    target.add_instruction(MCXGate, name="mcx")
    target.add_instruction(MCPhaseGate, name="mcphase")
    target.add_instruction(UnitaryGate, name="unitary")
    return target


def _random_seed() -> int:
    return secrets.randbits(32)


def _check_seed(seed) -> int:
    seed = int(seed)
    if not 0 <= seed < _SEED_LIMIT:
        raise QiskitError(f"seed must be in [0, 2**64), got {seed}")
    return seed


def classical_values(samples: np.ndarray, measured: dict[int, int]) -> np.ndarray:
    """Map sampled basis states to classical-register values.

    Bit ``c`` of each value is bit ``measured[c]`` of the sample; classical
    bits without a measurement are 0. Returns Python ints (object array),
    because a circuit may have more than 64 classical bits.
    """
    values, inverse = np.unique(samples, return_inverse=True)
    mapped = []
    for v in values.tolist():
        out = 0
        for clbit, qubit in measured.items():
            out |= ((v >> qubit) & 1) << clbit
        mapped.append(out)
    return np.array(mapped, dtype=object)[inverse.reshape(-1)]


class VikshepJob(JobV1):
    """A finished job: ``run`` computes the result before returning."""

    _async = False

    def __init__(self, backend, job_id, result):
        super().__init__(backend, job_id)
        self._result = result

    def submit(self):
        return

    def result(self, timeout=None):
        return self._result

    def status(self):
        return JobStatus.DONE


class VikshepSimulator(BackendV2):
    """Qiskit backend that runs circuits on the Vikshep Quantum CPU reference.

    Seeded counts and memory are bit-identical on every supported platform
    and for every thread count. Circuit ``i`` of a ``run`` call draws its
    shots from the Philox stream ``(seed_simulator, i)`` of VQS-1 section 8,
    so ``vikshep_quantum.Simulator.sample(shots, seed, i)`` reproduces them.

    Circuits need not be transpiled: gates outside the target are expanded
    with their Qiskit definitions. Measurements must come at the end of the
    circuit.

    Options: ``shots`` (default 1024), ``memory`` (default False),
    ``seed_simulator`` (default None: a random seed, reported in the result)
    and ``threads`` (default None: all cores; results do not depend on it).

    Each experiment result has ``metadata["manifest"]``, the canonical JSON
    run record of the final state, and ``metadata["stream"]``.
    """

    def __init__(self, provider=None, **fields):
        super().__init__(
            provider=provider,
            name="vikshep_simulator",
            description="Deterministic state-vector simulator (VQS-1 CPU reference)",
            backend_version=vq.__version__,
            **fields,
        )
        self._target = _build_target()

    @property
    def target(self) -> Target:
        return self._target

    @property
    def max_circuits(self):
        return None

    @classmethod
    def _default_options(cls) -> Options:
        return Options(shots=1024, memory=False, seed_simulator=None, threads=None)

    def run(self, run_input, **options) -> VikshepJob:
        """Run one circuit or a list of circuits. Options given here
        override the backend's options for this call."""
        opts = {key: getattr(self.options, key) for key in ("shots", "memory", "seed_simulator", "threads")}
        for key, value in options.items():
            if key in opts:
                opts[key] = value
            else:
                warnings.warn(f"Option {key} is not used by this backend", UserWarning, stacklevel=2)
        circuits = [run_input] if isinstance(run_input, QuantumCircuit) else list(run_input)
        shots = int(opts["shots"])
        if shots < 0:
            raise QiskitError(f"shots must be non-negative, got {shots}")
        seed = _random_seed() if opts["seed_simulator"] is None else _check_seed(opts["seed_simulator"])

        job_id = str(uuid.uuid4())
        start = time.time()
        results = [
            self._run_circuit(circuit, stream, shots, seed, bool(opts["memory"]), opts["threads"])
            for stream, circuit in enumerate(circuits)
        ]
        result = Result.from_dict(
            {
                "backend_name": self.name,
                "backend_version": self.backend_version,
                "job_id": job_id,
                "results": results,
                "status": "COMPLETED",
                "success": True,
                "time_taken": time.time() - start,
            }
        )
        return VikshepJob(self, job_id, result)

    @staticmethod
    def _run_circuit(circuit, stream, shots, seed, memory, threads) -> dict:
        start = time.time()
        converted = convert(circuit)
        sim = vq.Simulator(circuit.num_qubits, threads=threads)
        sim.run(converted.circuit)

        if circuit.num_clbits == 0:
            values = np.zeros(0, dtype=object)
        elif converted.measured:
            values = classical_values(sim.sample(shots, seed, stream), converted.measured)
        else:
            values = np.zeros(shots, dtype=object)
        keys, freq = np.unique(values, return_counts=True) if len(values) else ([], [])
        data = {"counts": {hex(int(k)): int(n) for k, n in zip(keys, freq, strict=True)}}
        if memory:
            data["memory"] = [hex(int(v)) for v in values]

        header = {
            "name": circuit.name,
            "n_qubits": circuit.num_qubits,
            "qreg_sizes": [[qreg.name, qreg.size] for qreg in circuit.qregs],
            "creg_sizes": [[creg.name, creg.size] for creg in circuit.cregs],
            "qubit_labels": [[qreg.name, j] for qreg in circuit.qregs for j in range(qreg.size)],
            "clbit_labels": [[creg.name, j] for creg in circuit.cregs for j in range(creg.size)],
            "memory_slots": circuit.num_clbits,
            "global_phase": float(circuit.global_phase) if not circuit.parameters else 0.0,
            "metadata": circuit.metadata if circuit.metadata is not None else {},
        }
        return {
            "name": circuit.name,
            "seed_simulator": seed,
            "shots": shots,
            "data": data,
            "status": "DONE",
            "success": True,
            "header": header,
            "metadata": {"stream": stream, "manifest": sim.manifest()},
            "time_taken": time.time() - start,
        }
