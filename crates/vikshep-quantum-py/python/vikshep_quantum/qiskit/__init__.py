"""Qiskit integration: a backend and V2 primitives on the VQS-1 CPU reference.

Install with ``pip install "vikshep-quantum[qiskit]"`` (Qiskit 2.0 or later).

Example::

    from qiskit import QuantumCircuit
    from vikshep_quantum.qiskit import VikshepSimulator, Sampler, Estimator

    qc = QuantumCircuit(2)
    qc.h(0)
    qc.cx(0, 1)
    qc.measure_all()

    counts = VikshepSimulator().run(qc, shots=1000, seed_simulator=7).result().get_counts()

Seeded counts, samples and expectation values have the same bits on every
supported platform and for every thread count. Circuits need not be
transpiled: gates with a VQS-1 equivalent run as one operation, and every
other gate is expanded with its Qiskit definition. Measurements must come
at the end of the circuit.
"""

try:
    import qiskit as _qiskit  # noqa: F401
except ImportError as err:  # pragma: no cover - depends on the environment
    raise ImportError(
        'vikshep_quantum.qiskit needs Qiskit: pip install "vikshep-quantum[qiskit]"'
    ) from err

from ._convert import from_qiskit
from .backend import VikshepJob, VikshepSimulator
from .primitives import Estimator, Sampler

__all__ = ["Estimator", "Sampler", "VikshepJob", "VikshepSimulator", "from_qiskit"]
