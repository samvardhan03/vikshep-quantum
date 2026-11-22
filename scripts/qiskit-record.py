"""Record seeded Qiskit workloads run through vikshep_quantum.qiskit.

    python -I scripts/qiskit-record.py OUT.json [TRANSPILED.json]

OUT.json holds, for fixed circuits and seeds: hashes of the counts and
per-shot memory of ``VikshepSimulator``, of the ``Sampler`` bit arrays, the
run manifests' hashes and the ``Estimator`` values as hex floats. CI writes
it on every platform and requires the files to be identical byte for byte.

TRANSPILED.json records the same circuits after ``transpile`` at
optimization levels 0 to 3 (a hash of each transpiled circuit in QPY and of
its results). It is compared for information only: Qiskit's optimization
passes compute new gate angles in floating point, and with Qiskit 2.6.2 and
this backend's gate set, levels 2 and 3 returned a different circuit on
each call even with a fixed ``seed_transpiler``.
"""

import hashlib
import io
import json
import math
import platform
import sys

import numpy as np
import qiskit
from qiskit import QuantumCircuit, qpy, transpile
from qiskit.circuit.library import QFTGate, efficient_su2
from qiskit.circuit.random import random_circuit
from qiskit.quantum_info import SparsePauliOp

import vikshep_quantum as vq
from vikshep_quantum.qiskit import Estimator, Sampler, VikshepSimulator


def sha(data) -> str:
    if not isinstance(data, bytes):
        data = json.dumps(data, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(data).hexdigest()


def bound(ansatz, seed):
    values = np.random.default_rng(seed).uniform(-math.pi, math.pi, ansatz.num_parameters)
    return ansatz.assign_parameters(values)


def circuits():
    ghz = QuantumCircuit(12, name="ghz_12")
    ghz.h(0)
    for q in range(11):
        ghz.cx(q, q + 1)
    rand = random_circuit(8, 20, max_operands=3, seed=7)
    rand.name = "random_8"
    qft = QuantumCircuit(10, name="qft_10")
    for q in range(10):
        qft.h(q)
        qft.rz(0.1 * (q + 1), q)
    qft.append(QFTGate(10), range(10))
    su2 = bound(efficient_su2(16, reps=3), 1)
    su2.name = "su2_16"
    out = [ghz, rand, qft, su2]
    for c in out:
        c.measure_all()
    return out


def run_record(backend, circs, shots, seed):
    result = backend.run(circs, shots=shots, seed_simulator=seed, memory=True).result()
    record = {}
    for i, c in enumerate(circs):
        data = result.results[i].data
        record[c.name] = {
            "counts_sha256": sha(data.counts),
            "memory_sha256": sha(data.memory),
            "manifest_hash": json.loads(result.results[i].metadata["manifest"])["manifest_hash"],
        }
    return record


def main(out_path, transpiled_path=None):
    backend = VikshepSimulator()
    circs = circuits()
    record = {
        "versions": {"vikshep_quantum": vq.__version__, "qiskit": qiskit.__version__},
        "backend": run_record(backend, circs, 4096, 1),
    }

    ansatz = efficient_su2(8, reps=2)
    ansatz.measure_all()
    sweep = np.random.default_rng(2).uniform(-math.pi, math.pi, (5, ansatz.num_parameters))
    pub = Sampler(seed=3).run([(ansatz, sweep)], shots=1000).result()[0]
    record["sampler"] = {
        "meas_sha256": sha(pub.data.meas.array.tobytes()),
        "manifest_hashes": pub.metadata["manifest_hashes"],
    }

    rng = np.random.default_rng(4)
    labels = ["".join(rng.choice(list("IXYZ"), 10)) for _ in range(20)]
    hamiltonian = SparsePauliOp(labels, rng.uniform(-1, 1, 20))
    est_ansatz = efficient_su2(10, reps=2)
    values = np.random.default_rng(5).uniform(-math.pi, math.pi, (4, est_ansatz.num_parameters))
    evs = Estimator().run([(est_ansatz, hamiltonian, values)]).result()[0].data.evs
    record["estimator"] = [float(v).hex() for v in evs]

    with open(out_path, "w", newline="\n") as f:
        json.dump(record, f, indent=1, sort_keys=True)
        f.write("\n")
    print(f"{out_path}: {sha(open(out_path, 'rb').read())} ({platform.platform()})")

    if transpiled_path:
        info = {"versions": record["versions"]}
        for level in (0, 1, 2, 3):
            tcircs = [transpile(c, backend, optimization_level=level, seed_transpiler=0) for c in circs]
            qpys = {}
            for t in tcircs:
                buf = io.BytesIO()
                qpy.dump(t, buf)
                qpys[t.name] = sha(buf.getvalue())
            runs = run_record(backend, tcircs, 4096, 1)
            info[f"level_{level}"] = {
                name: {"qpy_sha256": qpys[name], **runs[name]} for name in qpys
            }
        with open(transpiled_path, "w", newline="\n") as f:
            json.dump(info, f, indent=1, sort_keys=True)
            f.write("\n")
        print(f"{transpiled_path}: {sha(open(transpiled_path, 'rb').read())}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
