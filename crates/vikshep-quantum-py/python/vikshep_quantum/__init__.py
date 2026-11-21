"""Deterministic quantum state-vector simulator (VQS-1).

Circuits run on the CPU reference backend; states, probabilities, samples
and expectation values are bit-identical on every supported platform and
for every thread count. Qubit 0 is the least significant bit of a
basis-state index, as in Qiskit.

Example::

    import vikshep_quantum as vq

    bell = vq.Circuit(2).h(0).cx(0, 1)
    sim = vq.Simulator(2)
    sim.run(bell)
    vq.counts(sim.sample(1000, seed=7), 2)   # {'00': ..., '11': ...}
    sim.expectation("Z0 Z1")                 # 1.0 up to binary32 rounding
    sim.manifest()                           # canonical JSON run record
"""

import numpy as _np

from ._core import (
    MAX_QUBITS,
    NUMERICS_VERSION,
    PLATFORM,
    VQS_VERSION,
    Circuit,
    Simulator,
    __version__,
    conformance_selftest,
    oid,
)


def counts(samples, n_qubits):
    """Histogram of samples as ``{bitstring: count}``, qubit 0 rightmost."""
    values, freq = _np.unique(_np.asarray(samples, dtype=_np.uint64), return_counts=True)
    return {format(int(v), f"0{n_qubits}b"): int(c) for v, c in zip(values, freq)}


__all__ = [
    "MAX_QUBITS",
    "NUMERICS_VERSION",
    "PLATFORM",
    "VQS_VERSION",
    "Circuit",
    "Simulator",
    "__version__",
    "conformance_selftest",
    "counts",
    "oid",
]
