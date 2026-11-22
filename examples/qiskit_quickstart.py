"""Qiskit quickstart: counts from the backend, the V2 Sampler and Estimator,
and the run record. Every number printed is the same on every supported
machine.

    pip install "vikshep-quantum[qiskit]"
    python examples/qiskit_quickstart.py
"""

import json

import numpy as np
from qiskit import QuantumCircuit
from qiskit.circuit.library import efficient_su2
from qiskit.quantum_info import SparsePauliOp

from vikshep_quantum.qiskit import Estimator, Sampler, VikshepSimulator

bell = QuantumCircuit(2)
bell.h(0)
bell.cx(0, 1)

measured = bell.copy()
measured.measure_all()

# Backend: no transpiling needed. Circuit i of a run uses Philox stream i.
result = VikshepSimulator().run(measured, shots=1000, seed_simulator=7).result()
print("backend counts:", result.get_counts())
manifest = json.loads(result.results[0].metadata["manifest"])
print("manifest hash:", manifest["manifest_hash"])

# Sampler V2: the same seed and the same stream give the same shots.
pub = Sampler(seed=7).run([measured], shots=1000).result()[0]
print("sampler counts:", pub.data.meas.get_counts())

# Estimator V2: exact values, the same bits everywhere.
print("<ZZ> =", Estimator().run([(bell, "ZZ")]).result()[0].data.evs)

ansatz = efficient_su2(4, reps=2)
values = np.linspace(-1, 1, ansatz.num_parameters)
hamiltonian = SparsePauliOp(["ZZII", "IZZI", "IIZZ", "XXXX"], [1.0, 1.0, 1.0, 0.5])
energy = Estimator().run([(ansatz, hamiltonian, values)]).result()[0].data.evs
print("energy =", float(energy), "=", float(energy).hex())
