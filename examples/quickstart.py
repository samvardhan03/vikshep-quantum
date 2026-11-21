"""Quickstart: a GHZ state, its samples, an expectation value and the run
record. Every number printed is the same on every supported machine.

    python examples/quickstart.py
"""

import json

import vikshep_quantum as vq

N = 5

ghz = vq.Circuit(N).h(0)
for q in range(1, N):
    ghz.cx(q - 1, q)

sim = vq.Simulator(N)
sim.run(ghz)

samples = sim.sample(1000, seed=2026)
print("counts:", vq.counts(samples, N))  # only 00000 and 11111
print("<Z0 Z4> =", sim.expectation("Z0 Z4"))  # 1.0
print("<X0 X1 X2 X3 X4> =", sim.expectation("X0 X1 X2 X3 X4"))  # 1.0 up to rounding
print("state OID:", sim.state_oid())

manifest = json.loads(sim.manifest())
print("manifest hash:", manifest["manifest_hash"])
print("conformance:", vq.conformance_selftest()["failed"], "failures")
